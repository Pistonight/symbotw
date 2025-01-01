use error_stack::{Result, ResultExt};
use gimli::{
    DW_TAG_array_type, DW_TAG_base_type, DW_TAG_class_type, DW_TAG_const_type,
    DW_TAG_enumeration_type, DW_TAG_pointer_type, DW_TAG_ptr_to_member_type, DW_TAG_reference_type,
    DW_TAG_restrict_type, DW_TAG_structure_type, DW_TAG_subrange_type, DW_TAG_subroutine_type,
    DW_TAG_typedef, DW_TAG_union_type, DW_TAG_unspecified_type, DW_TAG_volatile_type, DwTag,
};

use crate::parsed::{NamespaceMap, TypeComp, TypeInfo, TypePrim, TypesStage0};

use super::unit::{bad, err_ctx, opt_ctx};
use super::{
    read_base_type, read_enum_type, read_struct_type, read_subroutine_type, read_union_type, Error,
    Node, UnitCtx, UnitOffset,
};

/// Recursively read the type of the node
pub fn read_types<'i>(
    node: Node<'i, '_, '_, '_>,
    unit: &UnitCtx<'_, 'i>,
    namespaces: &NamespaceMap<'i>,
    types: &mut TypesStage0,
) -> Result<(), Error> {
    let entry = node.entry();
    if is_type_tag(entry.tag()) {
        read_type_at_offset(entry.offset(), unit, namespaces, types)?;
    }
    unit.for_each_child(node, |child| read_types(child, unit, namespaces, types))?;

    Ok(())
}

pub fn is_type_tag(tag: DwTag) -> bool {
    matches!(
        tag,
        DW_TAG_structure_type
        | DW_TAG_class_type
        | DW_TAG_union_type
        | DW_TAG_enumeration_type
        // typedefs
        | DW_TAG_unspecified_type
        | DW_TAG_typedef
        // pointer
        | DW_TAG_pointer_type
        | DW_TAG_reference_type
        | DW_TAG_array_type
        // qualifier
        | DW_TAG_const_type
        | DW_TAG_volatile_type
        | DW_TAG_restrict_type
        // function
        | DW_TAG_subroutine_type
        | DW_TAG_ptr_to_member_type
        // base
        | DW_TAG_base_type
    )
}

pub fn read_type_at_offset<'i>(
    offset: UnitOffset,
    unit: &UnitCtx<'_, 'i>,
    namespaces: &NamespaceMap<'i>,
    types: &mut TypesStage0,
) -> Result<TypeInfo, Error> {
    let global_offset = unit.to_global_offset(offset).into();
    if let Some(info) = types.get(&global_offset) {
        return Ok(info.clone());
    }
    let entry = unit.entry_at(offset)?;
    let info = match entry.tag() {
        DW_TAG_structure_type | DW_TAG_class_type => {
            read_struct_type(&entry, unit, namespaces, types)?
        }
        DW_TAG_union_type => read_union_type(&entry, unit, namespaces, types)?,
        DW_TAG_enumeration_type => read_enum_type(&entry, unit, namespaces, types)?,
        DW_TAG_unspecified_type => {
            // guess
            let name = unit.get_entry_name(&entry)?;
            match name {
                // std::nullptr_t
                "decltype(nullptr)" => TypeInfo::Prim(TypePrim::U64),
                _ => {
                    let offset = unit.to_global_offset(offset);
                    return bad!(unit, offset, Error::UnspecifiedType)
                        .attach_printable(format!("the DW_AT_name is: {}", name));
                }
            }
        }
        DW_TAG_typedef => {
            match unit.get_entry_type_offset_optional(&entry)? {
                // typedef to void.. just use void
                None => TypeInfo::Prim(TypePrim::Void),
                Some(x) => {
                    let name = unit.get_namespaced_name(&entry, namespaces)?;
                    TypeInfo::Typedef(name, unit.to_global_offset(x).into())
                }
            }
        }
        // T* or T&
        DW_TAG_pointer_type | DW_TAG_reference_type => {
            // can have void*
            let ty_offset = unit.get_entry_type_global_offset(&entry)?;
            TypeInfo::pointer(ty_offset.into())
        }
        // modifiers that don't do anything..
        DW_TAG_const_type | DW_TAG_volatile_type | DW_TAG_restrict_type => {
            // is just T. Create the type and instruct stage 1 type resolver to merge it
            match unit.get_entry_type_offset_optional(&entry)? {
                None => TypeInfo::Prim(TypePrim::Void),
                Some(x) => {
                    let a = unit.to_global_offset(x);
                    let b = unit.to_global_offset(entry.offset());
                    types.add_merge(a, b);
                    read_type_at_offset(x, unit, namespaces, types)?
                }
            }
        }
        // T[n]
        DW_TAG_array_type => {
            let target = unit.get_entry_type_offset(&entry)?;
            let global_offset = unit.to_global_offset(offset);
            let mut tree = unit.tree_at(offset)?;
            let root = unit.root_of(offset, &mut tree)?;
            let mut children = root.children();
            let subrange = err_ctx!(unit, global_offset, Error::ReadChild, children.next())?;
            let subrange = opt_ctx!(unit, global_offset, Error::ExpectingChild, subrange)?;
            let subrange = subrange.entry();
            unit.check_tag(subrange, DW_TAG_subrange_type)?;
            let count = unit.get_entry_count(subrange)?;
            match count {
                Some(count) => {
                    TypeInfo::Comp(TypeComp::Array(unit.to_global_offset(target).into(), count))
                }
                None => {
                    // without count, just use ptr type
                    TypeInfo::pointer(unit.to_global_offset(target).into())
                }
            }
        }
        // T(*)(Args...)
        DW_TAG_subroutine_type => read_subroutine_type(&entry, unit)?,
        // PTM
        DW_TAG_ptr_to_member_type => {
            let this_ty = unit.get_entry_containing_type_offset(&entry)?;
            let this_ty = unit.to_global_offset(this_ty);
            let ty = unit.get_entry_type_offset(&entry)?;
            let entry = unit.entry_at(ty)?;
            if entry.tag() == DW_TAG_subroutine_type {
                // PTMF

                if let TypeInfo::Comp(TypeComp::Subroutine(sub)) =
                    read_subroutine_type(&entry, unit)?
                {
                    TypeInfo::Comp(TypeComp::Ptmf(this_ty.into(), sub))
                } else {
                    unreachable!()
                }
            } else {
                // just normal pointer
                TypeInfo::pointer(unit.to_global_offset(ty).into())
            }
        }
        DW_TAG_base_type => TypeInfo::Prim(read_base_type(&entry, unit)?),

        tag => {
            let global_offset = unit.to_global_offset(offset);
            return bad!(unit, global_offset, Error::UnexpectedTagForType(tag));
        }
    };

    types.insert(global_offset, info.clone());
    Ok(info)
}

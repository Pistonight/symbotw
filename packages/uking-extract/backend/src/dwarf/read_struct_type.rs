use error_stack::{Result, ResultExt};
use gimli::{
    DW_TAG_GNU_template_parameter_pack, DW_TAG_class_type, DW_TAG_enumeration_type,
    DW_TAG_formal_parameter, DW_TAG_inheritance, DW_TAG_member, DW_TAG_structure_type,
    DW_TAG_subprogram, DW_TAG_template_type_parameter, DW_TAG_template_value_parameter,
    DW_TAG_typedef, DW_TAG_union_type,
};

use crate::parsed::{
    MemberInfo, NamespaceMap, StructInfo, Subroutine, TypeInfo, TypesStage0, VfptrInfo, VtableInfo,
};

use super::unit::bad;
use super::{read_type_at_offset, Error, UnitCtx, DIE};

/// Read the DIE as a DW_TAG_structure_type or DW_TAG_class_type
pub fn read_struct_type<'i>(
    entry: &DIE<'i, '_, '_>,
    unit: &UnitCtx<'_, 'i>,
    namespaces: &NamespaceMap<'i>,
    types: &mut TypesStage0,
) -> Result<TypeInfo, Error> {
    // structs can be anonymous
    let name = unit.get_namespaced_name_optional(entry, namespaces)?;
    // is declaration?
    if unit.get_entry_declaration(entry)? {
        let size = unit.get_entry_byte_size_optional(entry)?;
        if name.is_none() {
            return bad!(
                unit,
                unit.to_global_offset(entry.offset()),
                Error::UnexpectedAnonymousDeclaration
            );
        }
        return Ok(TypeInfo::Struct(StructInfo {
            name,
            is_decl: true,
            vtable: VtableInfo::default(), //Vec::new(),
            size,
            members: Vec::new(),
        }));
    }
    let byte_size = unit.get_entry_byte_size_optional(entry)?;
    let mut vtable = VtableInfo::default();
    let mut vdtor = None;
    let mut members = Vec::<MemberInfo>::new();
    let mut is_first_base = true;
    unit.for_each_child_entry(entry, |child| {
        let entry = child.entry();
        match entry.tag() {
            DW_TAG_member => {
                if unit.get_entry_external(entry)? {
                    return Ok(());
                }
                let name = unit.get_entry_name_optional(entry)?.map(|s| s.to_string());
                let ty_offset = unit
                    .to_global_offset(unit.get_entry_type_offset(entry)?)
                    .into();
                let offset = unit.get_entry_data_member_location(entry)?;
                let mut info = MemberInfo {
                    offset,
                    name,
                    is_base: false,
                    ty_offset,
                    is_bitfield: false,
                    byte_size: 0,
                };

                if unit.get_entry_bit_size(entry)?.is_some() {
                    let byte_size = unit.get_entry_byte_size(entry)?;
                    // bitfield, may be collapsed with previous member
                    info.make_bitfield(byte_size);
                    if let Some(prev) = members.last_mut() {
                        if prev.offset == info.offset && prev.is_bitfield {
                            // collapse
                            *prev = info;
                            return Ok(());
                        }
                    }
                }
                members.push(info);
            }
            DW_TAG_inheritance => {
                let offset = unit.get_entry_data_member_location(entry)?;
                let ty_offset = unit.get_entry_type_offset(entry)?;
                // derived classes may not have the full vtable
                // so we need to copy the base vtable
                if is_first_base {
                    is_first_base = false;
                    let mut ty_offset = ty_offset;
                    loop {
                        match read_type_at_offset(ty_offset, unit, namespaces, types)? {
                            TypeInfo::Struct(base) => {
                                vtable.inherit_from_base(&base.vtable);
                                break;
                            }
                            TypeInfo::Typedef(_, ty) => ty_offset = unit.to_unit_offset(ty.into()),
                            _ => {
                                // transparent base type
                                break;
                            }
                        }
                    }
                }
                let name = if offset == 0 {
                    "base".to_string()
                } else {
                    format!("base_{:x}", offset)
                };
                members.push(MemberInfo {
                    offset,
                    name: Some(name),
                    is_base: true,
                    ty_offset: unit.to_global_offset(ty_offset).into(),
                    is_bitfield: false,
                    byte_size: 0,
                });
            }
            DW_TAG_subprogram => {
                if let Some(velem) = unit.get_entry_vtable_elem_location(entry)? {
                    let name = unit.get_entry_name(entry)?.to_string();
                    let retty_offset = unit.get_entry_type_global_offset(entry)?.into();
                    let mut argty_offsets = Vec::new();
                    unit.for_each_child_entry(entry, |child| {
                        let entry = child.entry();
                        if entry.tag() == DW_TAG_formal_parameter {
                            let ty_offset = unit.get_entry_type_global_offset(entry)?;
                            argty_offsets.push(ty_offset.into());
                        }
                        Ok(())
                    })?;
                    let vfptr = VfptrInfo {
                        name,
                        is_from_base: false,
                        function: Subroutine {
                            retty: retty_offset,
                            params: argty_offsets,
                        },
                    };

                    // dtors are weird that they are declared 0th, but might not actually be
                    if vfptr.is_dtor() {
                        if velem != 0 {
                            return bad!(
                                unit,
                                unit.to_global_offset(entry.offset()),
                                Error::ConflictingVtableEntry
                            )
                            .attach_printable(format!("Vtable entry {}", velem))
                            .attach_printable(format!("Vtable: {:?}", vtable))
                            .attach_printable(format!("Need to insert: {:?}", vfptr.name))
                            .attach_printable("Expecting dtor to be declared 0th in vtable");
                        }
                        vdtor = Some(vfptr);
                    } else {
                        let name = vfptr.name.clone();
                        if !vtable.set(velem, vfptr) {
                            return bad!(
                                unit,
                                unit.to_global_offset(entry.offset()),
                                Error::ConflictingVtableEntry
                            )
                            .attach_printable(format!("Vtable entry {}", velem))
                            .attach_printable(format!("Vtable: {:?}", vtable))
                            .attach_printable(format!("Need to insert: {:?}", name));
                        }
                    }
                }
            }
            DW_TAG_structure_type
            | DW_TAG_class_type
            | DW_TAG_union_type
            | DW_TAG_enumeration_type
            | DW_TAG_typedef
            | DW_TAG_template_type_parameter
            | DW_TAG_template_value_parameter
            | DW_TAG_GNU_template_parameter_pack => {
                // ignore subtypes
            }
            tag => {
                let global_offset = unit.to_global_offset(entry.offset());
                return bad!(unit, global_offset, Error::UnexpectedTag(tag))
                    .attach_printable("reading DW_TAG_structure_type");
            }
        }
        Ok(())
    })?;
    // place virtual dtor
    if let Some(v) = vdtor {
        vtable.place_dtor(v);
    }
    if let Some(i) = vtable.has_vacant() {
        return bad!(
            unit,
            unit.to_global_offset(entry.offset()),
            Error::MissingVtableEntry
        )
        .attach_printable(format!("Vtable entry {}", i))
        .attach_printable(format!("Vtable: {:?}", vtable));
    }
    // transparent struct simplification
    // if the struct has 1 member, and no vtable
    if members.len() == 1 && vtable.is_empty() {
        let member = &members[0];
        if member.offset == 0 {
            // making std structs transparent can merge things like std::array and primitive arrays
            let is_std_struct = match &name {
                Some(name) => name.starts_with("std::"),
                None => false,
            };
            // this guard is here because not all members can be made transparent
            // notably the recursive ones
            if name.is_none() || member.name.is_none() || is_std_struct {
                types.add_merge(member.ty_offset, unit.to_global_offset(entry.offset()));
                // this is necessary to eliminate the struct definition entirely
                // otherwise, we will end up with a struct containing itself as a single member
                match name {
                    None => {
                        // turn the struct into that member
                        let off = unit.to_unit_offset(member.ty_offset.into());
                        return read_type_at_offset(off, unit, namespaces, types);
                    }
                    Some(name) => {
                        // turn the struct into a typedef
                        let ty_offset = member.ty_offset;
                        return Ok(TypeInfo::Typedef(name, ty_offset));
                    }
                }
            }
        }
    }

    Ok(TypeInfo::Struct(StructInfo {
        name,
        is_decl: false,
        size: byte_size,
        members,
        vtable,
    }))
}

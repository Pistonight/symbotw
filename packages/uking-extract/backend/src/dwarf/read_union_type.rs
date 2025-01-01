use error_stack::{Result, ResultExt};
use gimli::{
    DW_TAG_GNU_template_parameter_pack, DW_TAG_class_type, DW_TAG_enumeration_type, DW_TAG_member,
    DW_TAG_structure_type, DW_TAG_subprogram, DW_TAG_template_type_parameter,
    DW_TAG_template_value_parameter, DW_TAG_typedef, DW_TAG_union_type,
};

use crate::parsed::{NamespaceMap, Offset, TypeInfo, TypesStage0, UnionInfo};

use super::unit::bad;
use super::{Error, UnitCtx, DIE};

/// Read a union type DIE (DW_TAG_union_type)
pub fn read_union_type<'i>(
    entry: &DIE<'i, '_, '_>,
    unit: &UnitCtx<'_, 'i>,
    namespaces: &NamespaceMap<'i>,
    _types: &mut TypesStage0,
) -> Result<TypeInfo, Error> {
    // can be anonymous
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
        return Ok(TypeInfo::Union(UnionInfo::decl(name, size)));
    }
    let byte_size = unit.get_entry_byte_size(entry)?;
    let mut members = Vec::<(Option<String>, Offset)>::new();
    unit.for_each_child_entry(entry, |child| {
        let entry = child.entry();
        match entry.tag() {
            DW_TAG_member => {
                let name = unit.get_entry_name_optional(entry)?.map(|s| s.to_string());
                let ty_offset = unit
                    .to_global_offset(unit.get_entry_type_offset(entry)?)
                    .into();
                // if type is duplicated, just ignore it
                if !members.iter().any(|x| x.1 == ty_offset) {
                    members.push((name, ty_offset));
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
            DW_TAG_subprogram => {
                // Disallow unions to be virtual
                if unit.get_entry_vtable_elem_location(entry)?.is_some() {
                    return bad!(
                        unit,
                        unit.to_global_offset(entry.offset()),
                        Error::UnexpectedVirtualUnion
                    );
                }
            }
            tag => {
                let global_offset = unit.to_global_offset(entry.offset());
                return bad!(unit, global_offset, Error::UnexpectedTag(tag))
                    .attach_printable("reading DW_TAG_union_type");
            }
        }
        Ok(())
    })?;

    // unions are optimized during stage 3, so we don't do optimizations here

    Ok(TypeInfo::Union(UnionInfo {
        name,
        size: byte_size,
        is_decl: false,
        members,
    }))
}

use gimli::DW_TAG_enumerator;

use error_stack::{Result, ResultExt};

use crate::parsed::{EnumInfo, NamespaceMap, TypeInfo, TypesStage0};

use super::unit::bad;
use super::{read_type_at_offset, Error, UnitCtx, DIE};

pub fn read_enum_type<'i>(
    entry: &DIE<'i, '_, '_>,
    unit: &UnitCtx<'_, 'i>,
    namespaces: &NamespaceMap<'i>,
    types: &mut TypesStage0,
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
        return Ok(TypeInfo::Enum(EnumInfo {
            name,
            is_decl: true,
            size,
            enumerators: Vec::new(),
        }));
    }
    let byte_size = match unit.get_entry_type_offset_optional(entry)? {
        Some(off) => {
            let mut off = off;
            loop {
                let ty = read_type_at_offset(off, unit, namespaces, types)?;
                match ty {
                    TypeInfo::Prim(p) => break p.size().unwrap(),
                    TypeInfo::Typedef(_, next) => {
                        off = unit.to_unit_offset(next.into());
                    }
                    _ => {
                        return bad!(unit, unit.to_global_offset(off), Error::UnexpectedEnumType)
                            .attach_printable(format!(
                                "base type is {}, which is not primitive",
                                ty
                            ))
                    }
                }
            }
        }
        None => unit.get_entry_byte_size(entry)?,
    };
    let mut members = Vec::new();
    unit.for_each_child_entry(entry, |child| {
        let entry = child.entry();
        match entry.tag() {
            DW_TAG_enumerator => {
                let name = unit.get_entry_name(entry)?;
                let value = unit.get_entry_const_value(entry)?;
                members.push((name.to_string(), value));
            }
            tag => {
                let global_offset = unit.to_global_offset(entry.offset());
                return bad!(unit, global_offset, Error::UnexpectedTag(tag))
                    .attach_printable("reading DW_TAG_enumeration_type");
            }
        }
        Ok(())
    })?;

    Ok(TypeInfo::Enum(EnumInfo {
        name,
        size: byte_size,
        is_decl: false,
        enumerators: members,
    }))
}

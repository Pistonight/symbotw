use crate::parsed::TypeInfo;

use error_stack::Result;
use gimli::DW_TAG_formal_parameter;

use super::{Error, UnitCtx, DIE};

pub fn read_subroutine_type<'i>(
    entry: &DIE<'i, '_, '_>,
    unit: &UnitCtx<'_, 'i>,
) -> Result<TypeInfo, Error> {
    let rettype = unit.get_entry_type_global_offset(entry)?;
    let mut arg_types = Vec::new();
    unit.for_each_child_entry(entry, |child| {
        let entry = child.entry();
        unit.check_tag(entry, DW_TAG_formal_parameter)?;
        let ty = unit.get_entry_type_global_offset(entry)?;
        arg_types.push(ty.into());
        Ok(())
    })?;

    Ok(TypeInfo::subroutine(rettype.into(), arg_types))
}

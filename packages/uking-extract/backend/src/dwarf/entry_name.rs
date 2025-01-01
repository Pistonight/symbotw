use gimli::DW_AT_name;

use error_stack::Result;

use super::unit::{err_ctx, opt_ctx};
use super::{Error, UnitCtx, DIE};

impl<'d, 'i> UnitCtx<'d, 'i> {
    /// Get the DW_AT_name of a DIE
    pub fn get_entry_name(&self, entry: &DIE<'i, '_, '_>) -> Result<&'i str, Error> {
        let offset = self.to_global_offset(entry.offset());
        let name = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(DW_AT_name),
            entry.attr_value(DW_AT_name)
        )?;
        let name = opt_ctx!(self, offset, Error::MissingEntryAttr(DW_AT_name), name)?;
        self.get_string(name)
    }

    /// Get the DW_AT_name of a DIE, allowing it to be missing
    pub fn get_entry_name_optional(
        &self,
        entry: &DIE<'i, '_, '_>,
    ) -> Result<Option<&'i str>, Error> {
        let offset = self.to_global_offset(entry.offset());
        let name = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(DW_AT_name),
            entry.attr_value(DW_AT_name)
        )?;
        match name {
            None => Ok(None),
            Some(name) => {
                let name = self.get_string(name)?;
                Ok(Some(name))
            }
        }
    }
}

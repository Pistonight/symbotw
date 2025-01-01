use gimli::{AttributeValue, DW_AT_containing_type, DW_AT_type};

use error_stack::Result;

use super::unit::{err_ctx, opt_ctx};
use super::{Error, UnitCtx, UnitOffset, DIE};

impl<'d, 'i> UnitCtx<'d, 'i> {
    /// Get the DW_AT_type converted to global offset, or `usize::MAX` if the type is void
    pub fn get_entry_type_global_offset(&self, entry: &DIE<'i, '_, '_>) -> Result<usize, Error> {
        match self.get_entry_type_offset_optional(entry)? {
            Some(offset) => Ok(self.to_global_offset(offset)),
            None => Ok(usize::MAX),
        }
    }

    /// Get the DW_AT_type of a DIE
    pub fn get_entry_type_offset(&self, entry: &DIE<'i, '_, '_>) -> Result<UnitOffset, Error> {
        let offset = self.to_global_offset(entry.offset());
        let type_value = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(DW_AT_type),
            entry.attr_value(DW_AT_type)
        )?;
        let type_value = opt_ctx!(
            self,
            offset,
            Error::MissingEntryAttr(DW_AT_type),
            type_value
        )?;
        let type_offset = match type_value {
            AttributeValue::UnitRef(offset) => Some(offset),
            _ => None,
        };
        let type_offset = opt_ctx!(
            self,
            offset,
            Error::BadEntryAttrType(DW_AT_type, "UnitRef"),
            type_offset
        )?;
        Ok(type_offset)
    }

    /// Get the DW_AT_type of a DIE, allowing it to be missing
    pub fn get_entry_type_offset_optional(
        &self,
        entry: &DIE<'i, '_, '_>,
    ) -> Result<Option<UnitOffset>, Error> {
        let offset = self.to_global_offset(entry.offset());
        let type_value = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(DW_AT_type),
            entry.attr_value(DW_AT_type)
        )?;
        let type_value = match type_value {
            Some(type_value) => type_value,
            None => return Ok(None),
        };
        let type_offset = match type_value {
            AttributeValue::UnitRef(offset) => Some(offset),
            _ => None,
        };
        let type_offset = opt_ctx!(
            self,
            offset,
            Error::BadEntryAttrType(DW_AT_type, "UnitRef"),
            type_offset
        )?;
        Ok(Some(type_offset))
    }

    /// Get the DW_AT_containing_type of a DIE
    pub fn get_entry_containing_type_offset(
        &self,
        entry: &DIE<'i, '_, '_>,
    ) -> Result<UnitOffset, Error> {
        let offset = self.to_global_offset(entry.offset());
        let type_value = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(DW_AT_containing_type),
            entry.attr_value(DW_AT_containing_type)
        )?;
        let type_value = opt_ctx!(
            self,
            offset,
            Error::MissingEntryAttr(gimli::DW_AT_containing_type),
            type_value
        )?;
        let type_offset = match type_value {
            AttributeValue::UnitRef(offset) => Some(offset),
            _ => None,
        };
        let type_offset = opt_ctx!(
            self,
            offset,
            Error::BadEntryAttrType(DW_AT_containing_type, "UnitRef"),
            type_offset
        )?;
        Ok(type_offset)
    }
}

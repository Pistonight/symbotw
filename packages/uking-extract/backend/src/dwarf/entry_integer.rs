use gimli::{
    AttributeValue, DW_AT_bit_size, DW_AT_byte_size, DW_AT_const_value, DW_AT_count,
    DW_AT_data_member_location, DwAt, Operation,
};

use error_stack::{Result, ResultExt};

use super::unit::{bad, err_ctx, opt_ctx};
use super::{Error, In, UnitCtx, DIE};

impl<'d, 'i> UnitCtx<'d, 'i> {
    /// Get the DW_AT_count of a DIE
    pub fn get_entry_count(&self, entry: &DIE<'i, '_, '_>) -> Result<Option<usize>, Error> {
        Ok(self
            .get_entry_unsigned_attr_optional(entry, DW_AT_count)?
            .map(|x| x.try_into().unwrap()))
    }

    /// Get the DW_AT_byte_size of a DIE
    pub fn get_entry_byte_size(&self, entry: &DIE<'i, '_, '_>) -> Result<usize, Error> {
        Ok(self
            .get_entry_unsigned_attr(entry, DW_AT_byte_size)?
            .try_into()
            .unwrap())
    }

    /// Get the DW_AT_byte_size of a DIE, allowing it to be missing
    pub fn get_entry_byte_size_optional(&self, entry: &DIE<'i, '_, '_>) -> Result<usize, Error> {
        match self.get_entry_unsigned_attr_optional(entry, DW_AT_byte_size)? {
            Some(x) => Ok(x.try_into().unwrap()),
            None => Ok(0),
        }
    }

    /// Get the DW_AT_data_member_location of a DIE
    pub fn get_entry_data_member_location(&self, entry: &DIE<'i, '_, '_>) -> Result<usize, Error> {
        Ok(self
            .get_entry_unsigned_attr(entry, DW_AT_data_member_location)?
            .try_into()
            .unwrap())
    }

    /// Get the DW_AT_bit_size of a DIE
    pub fn get_entry_bit_size(&self, entry: &DIE<'i, '_, '_>) -> Result<Option<usize>, Error> {
        Ok(self
            .get_entry_unsigned_attr_optional(entry, DW_AT_bit_size)?
            .map(|x| x.try_into().unwrap()))
    }

    /// Get the DW_AT_const_value of a DIE
    pub fn get_entry_const_value(&self, entry: &DIE<'i, '_, '_>) -> Result<i128, Error> {
        self.get_entry_signed_attr(entry, DW_AT_const_value)
    }

    /// Get a signed integer attribute value
    pub fn get_entry_signed_attr(
        &self,
        entry: &DIE<'i, '_, '_>,
        attr: DwAt,
    ) -> Result<i128, Error> {
        let offset = self.to_global_offset(entry.offset());
        let value = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(attr),
            entry.attr_value(attr)
        )?;
        let value = opt_ctx!(self, offset, Error::MissingEntryAttr(attr), value)?;
        let value = self.get_signed(offset, attr, value)?;
        Ok(value)
    }

    /// Get a signed integer attribute value, allowing it to be missing
    #[allow(dead_code)]
    pub fn get_entry_signed_attr_optional(
        &self,
        entry: &DIE<'i, '_, '_>,
        attr: DwAt,
    ) -> Result<Option<i128>, Error> {
        let offset = self.to_global_offset(entry.offset());
        let value = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(attr),
            entry.attr_value(attr)
        )?;
        match value {
            Some(value) => {
                let value = self.get_signed(offset, attr, value)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Get an unsigned integer attribute value
    pub fn get_entry_unsigned_attr(
        &self,
        entry: &DIE<'i, '_, '_>,
        attr: DwAt,
    ) -> Result<u64, Error> {
        let offset = self.to_global_offset(entry.offset());
        let value = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(attr),
            entry.attr_value(attr)
        )?;
        let value = opt_ctx!(self, offset, Error::MissingEntryAttr(attr), value)?;
        let value = self.get_unsigned(offset, attr, value)?;
        Ok(value)
    }

    /// Get an unsigned integer attribute value, allowing it to be missing
    pub fn get_entry_unsigned_attr_optional(
        &self,
        entry: &DIE<'i, '_, '_>,
        attr: DwAt,
    ) -> Result<Option<u64>, Error> {
        let offset = self.to_global_offset(entry.offset());
        let value = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(attr),
            entry.attr_value(attr)
        )?;
        match value {
            Some(value) => {
                let value = self.get_unsigned(offset, attr, value)?;
                Ok(Some(value))
            }
            None => Ok(None),
        }
    }

    /// Get an attribute value as signed integer
    pub fn get_signed(
        &self,
        global_offset: usize,
        at: DwAt,
        attr: AttributeValue<In<'i>>,
    ) -> Result<i128, Error> {
        match attr {
            AttributeValue::Data1(x) => Ok(x as i128),
            AttributeValue::Data2(x) => Ok(x as i128),
            AttributeValue::Data4(x) => Ok(x as i128),
            AttributeValue::Data8(x) => Ok(x as i128),
            AttributeValue::Udata(x) => Ok(x as i128),
            AttributeValue::Sdata(x) => Ok(x as i128),
            _ => bad!(self, global_offset, Error::BadEntryAttrType(at, "int data"))
                .attach_printable(format!("Got: {:?}", attr)),
        }
    }
    /// Get an attribute value as unsigned integer
    pub fn get_unsigned(
        &self,
        global_offset: usize,
        at: DwAt,
        attr: AttributeValue<In<'i>>,
    ) -> Result<u64, Error> {
        match attr {
            AttributeValue::Data1(x) => Ok(x as u64),
            AttributeValue::Data2(x) => Ok(x as u64),
            AttributeValue::Data4(x) => Ok(x as u64),
            AttributeValue::Data8(x) => Ok(x),
            AttributeValue::Udata(x) => Ok(x),
            // this is used for vtable elem location
            AttributeValue::Exprloc(expr) => {
                let mut ops = expr.operations(self.unit.encoding());
                if let Some(op) =
                    err_ctx!(self, global_offset, Error::ReadEntryAttr(at), ops.next())?
                {
                    match op {
                        Operation::UnsignedConstant { value } => Ok(value),
                        _ => bad!(self, global_offset, Error::ReadEntryAttr(at))
                            .attach_printable("Expecting an unsigned constant operation"),
                    }
                } else {
                    bad!(self, global_offset, Error::ReadEntryAttr(at))
                        .attach_printable("Expecting an operation")
                }
            }
            _ => bad!(
                self,
                global_offset,
                Error::BadEntryAttrType(at, "unsigned data")
            )
            .attach_printable(format!("Got: {:?}", attr)),
        }
    }
}

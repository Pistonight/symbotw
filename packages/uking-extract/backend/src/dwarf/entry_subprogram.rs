use error_stack::{Result, ResultExt};
use gimli::{
    AttributeValue, DW_AT_abstract_origin, DW_AT_inline, DW_AT_linkage_name, DW_AT_low_pc,
    DW_AT_specification, DW_AT_virtuality, DW_AT_vtable_elem_location, DW_INL_declared_inlined,
    DW_INL_declared_not_inlined, DW_INL_inlined, DW_INL_not_inlined, DW_VIRTUALITY_none,
    DW_VIRTUALITY_pure_virtual, DW_VIRTUALITY_virtual,
};

use super::unit::{bad, err_ctx, opt_ctx};
use super::{Error, In, UnitCtx, UnitOffset, DIE};

impl<'d, 'i> UnitCtx<'d, 'i> {
    /// Get the DW_TAG_vtable_elem_location of a DIE, return None if not virtual
    pub fn get_entry_vtable_elem_location(
        &self,
        entry: &DIE<'i, '_, '_>,
    ) -> Result<Option<usize>, Error> {
        let offset = self.to_global_offset(entry.offset());
        let virtuality = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(DW_AT_virtuality),
            entry.attr_value(DW_AT_virtuality)
        )?;
        match virtuality {
            None | Some(AttributeValue::Virtuality(DW_VIRTUALITY_none)) => {
                let vel = err_ctx!(
                    self,
                    offset,
                    Error::ReadEntryAttr(DW_AT_vtable_elem_location),
                    entry.attr_value(DW_AT_vtable_elem_location)
                )?;
                if vel.is_some() {
                    return bad!(
                        self,
                        offset,
                        Error::UnexpectedEntryAttr(DW_AT_vtable_elem_location)
                    );
                }
                Ok(None)
            }
            Some(AttributeValue::Virtuality(DW_VIRTUALITY_virtual))
            | Some(AttributeValue::Virtuality(DW_VIRTUALITY_pure_virtual)) => {
                let vel = err_ctx!(
                    self,
                    offset,
                    Error::ReadEntryAttr(DW_AT_vtable_elem_location),
                    entry.attr_value(DW_AT_vtable_elem_location)
                )?;
                let vel = opt_ctx!(
                    self,
                    offset,
                    Error::MissingEntryAttr(DW_AT_vtable_elem_location),
                    vel
                )?;
                let vel = self.get_unsigned(offset, DW_AT_vtable_elem_location, vel)?;
                Ok(Some(vel.try_into().unwrap()))
            }
            _ => {
                bad!(
                    self,
                    offset,
                    Error::BadEntryAttrType(DW_AT_virtuality, "Virtuality")
                )
            }
        }
    }

    /// Get the DW_AT_linkage_name of a DIE, allowing it to be missing
    pub fn get_entry_linkage_name(
        &self,
        entry: &DIE<'i, '_, '_>,
    ) -> Result<Option<&'i str>, Error> {
        let offset = self.to_global_offset(entry.offset());
        let name = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(DW_AT_linkage_name),
            entry.attr_value(DW_AT_linkage_name)
        )?;
        match name {
            None => Ok(None),
            Some(name) => {
                let name = self.get_string(name)?;
                Ok(Some(name))
            }
        }
    }

    /// Get the DW_AT_low_pc of a DIE, allowing it to be missing
    pub fn get_entry_low_pc(&self, entry: &DIE<'i, '_, '_>) -> Result<Option<u64>, Error> {
        let offset = self.to_global_offset(entry.offset());
        let low_pc = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(DW_AT_low_pc),
            entry.attr_value(DW_AT_low_pc)
        )?;
        match low_pc {
            None => Ok(None),
            Some(low_pc) => {
                let low_pc = self.get_address(low_pc)?;
                Ok(low_pc)
            }
        }
    }

    /// Get the DW_AT_abstract_origin of a DIE, allowing it to be missing
    pub fn get_entry_abstract_origin(
        &self,
        entry: &DIE<'i, '_, '_>,
    ) -> Result<Option<UnitOffset>, Error> {
        let offset = self.to_global_offset(entry.offset());
        let origin = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(DW_AT_abstract_origin),
            entry.attr_value(DW_AT_abstract_origin)
        )?;
        match origin {
            None => Ok(None),
            Some(AttributeValue::UnitRef(offset)) => Ok(Some(offset)),
            _ => bad!(
                self,
                offset,
                Error::BadEntryAttrType(DW_AT_abstract_origin, "UnitRef")
            )
            .attach_printable(format!("Got: {:?}", origin)),
        }
    }

    /// Get the DW_AT_specification of a DIE, allowing it to be missing
    pub fn get_entry_specification(
        &self,
        entry: &DIE<'i, '_, '_>,
    ) -> Result<Option<UnitOffset>, Error> {
        let offset = self.to_global_offset(entry.offset());
        let origin = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(DW_AT_specification),
            entry.attr_value(DW_AT_specification)
        )?;
        match origin {
            None => Ok(None),
            Some(AttributeValue::UnitRef(offset)) => Ok(Some(offset)),
            _ => bad!(
                self,
                offset,
                Error::BadEntryAttrType(DW_AT_specification, "UnitRef")
            )
            .attach_printable(format!("Got: {:?}", origin)),
        }
    }

    #[allow(dead_code)]
    pub fn get_entry_inlined(&self, entry: &DIE<'i, '_, '_>) -> Result<bool, Error> {
        let offset = self.to_global_offset(entry.offset());
        let inline = err_ctx!(
            self,
            offset,
            Error::ReadEntryAttr(DW_AT_inline),
            entry.attr_value(DW_AT_inline)
        )?;
        match inline {
            None
            | Some(AttributeValue::Inline(DW_INL_not_inlined))
            | Some(AttributeValue::Inline(DW_INL_declared_not_inlined)) => Ok(false),
            Some(AttributeValue::Inline(DW_INL_inlined))
            | Some(AttributeValue::Inline(DW_INL_declared_inlined)) => Ok(true),
            _ => bad!(
                self,
                offset,
                Error::BadEntryAttrType(DW_AT_virtuality, "Virtuality")
            )
            .attach_printable(format!("Got: {:?}", inline)),
        }
    }

    /// Get an attribute value as address
    pub fn get_address(&self, attr: AttributeValue<In<'i>>) -> Result<Option<u64>, Error> {
        err_ctx!(
            self,
            Error::AttrAddress,
            self.dwarf.attr_address(&self.unit, attr)
        )
    }
}

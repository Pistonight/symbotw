use error_stack::{Result, ResultExt};
use gimli::{
    AttributeValue, DW_ATE_boolean, DW_ATE_float, DW_ATE_signed, DW_ATE_signed_char,
    DW_ATE_unsigned, DW_ATE_unsigned_char, DW_AT_byte_size, DW_AT_encoding, DW_ATE_UTF,
};

use crate::parsed::TypePrim;

use super::unit::{bad, err_ctx, opt_ctx};
use super::{Error, UnitCtx, DIE};

/// Get TypeInfo for a DW_TAG_base_type
pub fn read_base_type<'i>(
    entry: &DIE<'i, '_, '_>,
    unit: &UnitCtx<'_, 'i>,
) -> Result<TypePrim, Error> {
    let offset = unit.to_global_offset(entry.offset());
    let encoding = err_ctx!(
        unit,
        offset,
        Error::ReadEntryAttr(DW_AT_encoding),
        entry.attr_value(DW_AT_encoding)
    )?;
    let encoding = opt_ctx!(
        unit,
        offset,
        Error::MissingEntryAttr(DW_AT_encoding),
        encoding
    )?;
    let encoding = match encoding {
        AttributeValue::Encoding(x) => Some(x),
        _ => None,
    };
    let encoding = opt_ctx!(
        unit,
        offset,
        Error::BadEntryAttrType(DW_AT_encoding, "Encoding"),
        encoding
    )?;
    let byte_size = unit.get_entry_byte_size(entry)?;
    match (encoding, byte_size) {
        (DW_ATE_boolean, 0x1) => Ok(TypePrim::Bool),
        (DW_ATE_unsigned, 0x1) => Ok(TypePrim::U8),
        (DW_ATE_unsigned_char, 0x1) => Ok(TypePrim::U8),
        (DW_ATE_signed, 0x1) => Ok(TypePrim::I8),
        (DW_ATE_signed_char, 0x1) => Ok(TypePrim::I8),

        (DW_ATE_unsigned, 0x2) => Ok(TypePrim::U16),
        (DW_ATE_signed, 0x2) => Ok(TypePrim::I16),
        (DW_ATE_UTF, 0x2) => Ok(TypePrim::U16),

        (DW_ATE_unsigned, 0x4) => Ok(TypePrim::U32),
        (DW_ATE_signed, 0x4) => Ok(TypePrim::I32),
        (DW_ATE_float, 0x4) => Ok(TypePrim::F32),

        (DW_ATE_unsigned, 0x8) => Ok(TypePrim::U64),
        (DW_ATE_signed, 0x8) => Ok(TypePrim::I64),
        (DW_ATE_float, 0x8) => Ok(TypePrim::F64),

        (DW_ATE_unsigned, 0x10) => Ok(TypePrim::U128),
        (DW_ATE_signed, 0x10) => Ok(TypePrim::I128),
        (DW_ATE_float, 0x10) => Ok(TypePrim::F128),
        _ => {
            // TODO complete list above
            bad!(
                unit,
                offset,
                Error::BadEntryAttrTypes(
                    [DW_AT_encoding.to_string(), DW_AT_byte_size.to_string(),].join(", ")
                )
            )
            .attach_printable(format!("Got: encoding={}, bytes={}", encoding, byte_size))
        }
    }
}

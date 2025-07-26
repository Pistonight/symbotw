
use std::path::PathBuf;

use crate::{bail, Result, Context as _};

/// Parse trait for common formats used in command line
///
/// This is different from `FromStr` in standard library
pub trait Parse where Self: Sized {
    type Output;
    fn parse(x: String) -> Result<Self::Output>;
}

// /// Macro for adding blanket implementation using `FromStr`
// ///
// /// Usage: `cu::impl_parse!(MyType)`, where `MyType` implements `FromStr`
// macro_rules! impl_parse {
//     ($type:ty) => {
//         // TODO - proc macro needed
//     };
// }

impl Parse for String {
    type Output = Self;
    fn parse(x: String) -> Result<Self> {
        Ok(x)
    }
}

impl Parse for PathBuf {
    type Output = Self;
    fn parse(x: String) -> Result<Self> {
        Ok(x.into())
    }
}

// TODO: wrapper parser for json/yaml/toml formats

// bool: empty, 0/1, false/true, case-insensitive
impl Parse for bool {
    type Output = Self;
    fn parse(mut x: String) -> Result<Self> {
        x.make_ascii_lowercase();
        match x.as_str().trim() {
            "" | "0" | "false" => Ok(false),
            "1" | "true" => Ok(true),
            _ => bail!("failed to parse bool from `{x}`")
        }
    }
}

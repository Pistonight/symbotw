// anyhow re-exports
pub use anyhow::{self as __anyhow, bail, ensure, Context, Result};
// thiserror re-exports
pub use thiserror::Error as DeriveError;

macro_rules! format_log_error {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::error!("{msg}");
        msg
    }}
}
macro_rules! bail_log {
    ($($arg:tt)*) => {{
        let msg = format!($($arg)*);
        $crate::error!("{msg}");
        $crate::bail!(msg)
    }};
}

pub use log::error;

mod env_var;
pub use env_var::*;
mod parse;
pub use parse::*;

mod clap;
mod print;

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

mod env_var;
pub use env_var::*;
mod parse;
pub use parse::*;

mod clap;

pub use log::{info, error, warn, debug, trace};

/// Printing utils
mod print;

pub use print::{
    init_print_options, color_enabled, set_thread_print_name,
    progress_bar, progress_bar_lowp,
    ColorLevel, PrintLevel, ProgressBarHandle,
};

#[doc(hidden)]
pub mod __priv {
    pub use super::print::{__print_with_type, __PrintType};
}

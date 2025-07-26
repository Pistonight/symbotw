use std::path::PathBuf;

use crate::{bail, Context as _, Parse, Result};

/// Read an environment variable and automatically parses it
///
/// If the var is not set, it's treated as an empty string
pub fn env_var<T: Parse>(var: impl AsRef<str>) -> Result<T::Output> {
    let var = var.as_ref();
    log::trace!("reading env var `{var}`");
    let value = match std::env::var(var) {
        Ok(v) => v,
        Err(std::env::VarError::NotPresent) => String::new(),
        Err(e) => {
            log::error!("failed to read env var `{var}`: {e}");
            Err(e).context(format!("failed to read env var `{var}`"))?
        }
    };
    T::parse(value).with_context(|| {
        format_log_error!("failed to parse env var `{var}`")
    })
}


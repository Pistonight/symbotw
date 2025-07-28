use std::path::Path;

use crate::{Context, PathExtension};


/// Like `std::fs::read`, but shows the path in the error
pub fn read(path: impl AsRef<Path>) -> crate::Result<Vec<u8>> {
    let path = path.as_ref();
    std::fs::read(path).with_context(|| {
        format!("failed to read {} as bytes", path.display())
    })
}

/// Like `std::fs::write`, but creates directories if they don't exist, and shows the path in the error
pub fn write(path: impl AsRef<Path>, content: impl AsRef<[u8]>) -> crate::Result<()> {
    let path = path.as_ref();
    let content = content.as_ref();
    let Err(x) = std::fs::write(path, content) else {
        return Ok(())
    };
    if x.kind() == std::io::ErrorKind::NotFound {
        // ensure parent dir exists and try again
        if let Ok(parent) = path.parent_abs() {
            if !parent.exists() {
                ensure_dir(&parent).with_context(|| {
                    format!("could not automatically create parent directory: {}", parent.display())
                })?;
                // try again
                std::fs::write(path, content).with_context(|| {
                    format!("failed to write to {}", path.display())
                })?;
                return Ok(());
            }
        }
    }
    if x.kind() == std::io::ErrorKind::NotADirectory {
        if let Ok(parent) = path.parent_abs() {
            if parent.exists() && !parent.is_dir(){
                return Err(x).context(format!("the parent path exists and is not a directory: {}", path.display()))?;
            }
        }
    }
    return Err(x).context(format!("failed to write to {}", path.display()))?;
}

/// Like `std::fs::read_to_string`, but shows the path in the error
pub fn read_string(path: impl AsRef<Path>) -> crate::Result<String> {
    let path = path.as_ref();
    std::fs::read_to_string(path).with_context(|| {
        format!("failed to read {} as string", path.display())
    })
}

/// Ensure `path` exists and is a directory, creating one if not
pub fn ensure_dir(path: impl AsRef<Path>) -> crate::Result<()> {
    let path = path.as_ref();
    let exists = path.exists();
    if exists && !path.is_dir() {
        crate::bail!("{} exists and is not a directory or not accessible", path.display());
    }
    if !exists {
        std::fs::create_dir_all(path).with_context(|| {
        format!("failed to create directory at {}", path.display())
        })?
    }
    if !path.exists() || !path.is_dir() {
        crate::bail!("directory creation failed for: {}", path.display());
    }
    Ok(())
}

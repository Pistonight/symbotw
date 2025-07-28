use std::borrow::Cow;
use std::path::{Path, PathBuf};

use crate::Context as _;

/// Extension to paths
pub trait PathExtension {
    /// Get file name. Error if the file name is not UTF-8 or other error occurs
    fn file_name_str(&self) -> crate::Result<&str>;

    /// Get absolute path for a path.
    ///
    /// This is different from `canonicalize()` since that fails for paths that don't
    /// exist. Here, we do a fallback that normalizes the path manually if `canonicalize()`
    /// fails:
    /// - If the path is absolute, then empty, `.`, and `..` path segments are normalized and removed,
    ///   and error if it tries to get the parent of root at some point.
    /// - If the path is relative, then the path is normalized the same way as above,
    ///   and then appended to the absolute path of the current working directory.
    ///
    /// On Windows only, it returns the most compatible form using the `dunce` crate instead of
    /// UNC, and the drive letter is also normalized to upper case
    fn normalize(&self) -> crate::Result<PathBuf>;

    /// Get the parent path as an absolute path
    ///
    /// Path navigation is very complex and that's why we are paying a little performance
    /// cost and always returning `PathBuf`, and always converting the path to absolute.
    ///
    /// For path manipulating (i.e. as a OsStr), instead of navigation, use std `parent()`
    /// instead
    fn parent_abs(&self) -> crate::Result<PathBuf> {
        self.parent_abs_times(1)
    }
    
    /// Effecitvely chaining `parent_abs` `x` times
    fn parent_abs_times(&self, x: usize) -> crate::Result<PathBuf>;

    /// Try converting self to a relative path from current working directory.
    ///
    /// Return the path itself unmodified if conversion failed
    fn try_to_rel(&self) -> Cow<'_, Path> {
        self.try_to_rel_from(".")
    }

    /// Try converting self to a relative path from a base path
    fn try_to_rel_from(&self, path: impl AsRef<Path>) -> Cow<'_, Path>;
}

impl PathExtension for Path {
    fn file_name_str(&self) -> crate::Result<&str> {
        let file_name = self.file_name().with_context(|| {
            format!("cannot get file name for path: {}", self.display())
        })?;
        // to_str is ok on all platforms, because Rust internally
        // represent OsStrings on Windows as WTF-8
        // see https://doc.rust-lang.org/src/std/sys_common/wtf8.rs.html
        let Some(file_name) = file_name.to_str() else {
            crate::bail!("file name is not valid UTF-8: {}", self.display());
        };
        Ok(file_name)
    }

    fn normalize(&self) -> crate::Result<PathBuf> {
        if let Ok(x) = dunce::canonicalize(self) {
            return Ok(x);
        };
        if self.is_absolute() {
            return fallback_normalize_absolute(self);
        }

        let Ok(mut base) = dunce::canonicalize(".") else {
            crate::warn!("failed to normalize current directory");
            crate::bail!("cannot normalize current directory when normalizing relative path: {}", self.display());
        };

        base.push(self);
        fallback_normalize_absolute(&base)
    }

    fn parent_abs_times(&self, x: usize) -> crate::Result<PathBuf> {
        let mut out = self.normalize()?;
        
        // this is correct since out is normalized
        for _ in 0..x {
            if !out.pop() {
                crate::bail!("trying to get parent of root");
            }
        }
        Ok(out)
    }

    fn try_to_rel_from(&self, path: impl AsRef<Path>) -> Cow<'_, Path> {
        let path = path.as_ref();
        let res = match (self.is_absolute(), path.is_absolute()) {
            (true, true) => {
                pathdiff::diff_paths(self, path)
            },
            (true, false) => {
                let Ok(base) = path.normalize() else {
                    return Cow::Borrowed(self)
                };
                pathdiff::diff_paths(self, base.as_path())
            },
            (false, true) => {
                let Ok(self_) = self.normalize() else {
                    return Cow::Borrowed(self)
                };
                pathdiff::diff_paths(self_.as_path(), path)
            },
            (false, false) => {
                let Ok(self_) = self.normalize() else {
                    return Cow::Borrowed(self)
                };
                let Ok(base) = path.normalize() else {
                    return Cow::Borrowed(self)
                };
                pathdiff::diff_paths(self_.as_path(), base.as_path())
            }
        };
        match res {
            None => Cow::Borrowed(self),
            Some(x) => Cow::Owned(x)
        }
    }
}

/// Normalize a path that failed to canonicalize
fn fallback_normalize_absolute(path: &Path) -> crate::Result<PathBuf> {
    let mut prefix = None;
    let mut components = vec![];
    for c in path.components() {
        match c {
            std::path::Component::Prefix(_prefix) => {
                prefix = Some(_prefix);
            }
            std::path::Component::RootDir => {
                components.clear();
            },
            std::path::Component::CurDir => {},
            std::path::Component::ParentDir => {
                if components.pop().is_none() {
                    crate::bail!("trying to get parent of root when normalizing: {}", path.display());
                }
            },
            std::path::Component::Normal(os_str) => {
                components.push(os_str)
            }
        }
    }
    let mut out = match prefix {
        None => PathBuf::from("/"),
        Some(prefix) => {
            PathBuf::from(prefix.as_os_str().to_ascii_uppercase())
        }
    };
    out.extend(components);
    // simplify windows UNC if needed
    if out.as_os_str().as_encoded_bytes().starts_with(b"\\\\") {
        Ok(dunce::simplified(&out).to_path_buf())
    } else {
        Ok(out)
    }
}

macro_rules! impl_for_as_ref_path {
    ($type:ty) => { impl PathExtension for $type {
        fn file_name_str(&self) -> crate::Result<&str> {
        AsRef::<Path>::as_ref(self).file_name_str()
        }
        fn normalize(&self) -> crate::Result<PathBuf> {
        AsRef::<Path>::as_ref(self).normalize()
        }
        fn parent_abs_times(&self, x: usize) -> crate::Result<PathBuf> {
        AsRef::<Path>::as_ref(self).parent_abs_times(x)
        }
        fn try_to_rel_from(&self, path: impl AsRef<Path>) -> Cow<'_, Path> {
        AsRef::<Path>::as_ref(self).try_to_rel_from(path)
        }
    } }
}

impl_for_as_ref_path!(PathBuf);

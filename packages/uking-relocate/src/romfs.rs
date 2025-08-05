use std::path::{Path, PathBuf};

use cu::pre::*;

pub struct Romfs {
    /// Path to Actor/ActorInfo.product.sbyml (or .byml)
    pub actor_info: PathBuf,
}

impl Romfs {
    pub fn find_paths(
        sdk_path: impl AsRef<Path>,
        romfs_path: Option<&str>,
    ) -> cu::Result<Self> {
        let sdk_path = sdk_path.as_ref();
        let exefs_dir = sdk_path.parent_abs().context("failed to find romfs directory")?;
        let actor_info = find_romfs_file(&exefs_dir, romfs_path, "Actor/ActorInfo.product.sbyml")?;
        Ok(Self { actor_info })
    }

    pub fn load_actor_info_data(&self) -> cu::Result<Vec<u8>> {
        cu::info!("loading romfs actor info");
        let bytes = cu::fs::read(&self.actor_info)?;
        let decompressed_bytes = roead::yaz0::decompress_if(&bytes);
        Ok(decompressed_bytes.to_vec())
    }
}

fn find_romfs_file(base: &Path, romfs_base: Option<&str>, file: &str) -> cu::Result<PathBuf> {
    // if a romfs base is given, then we just use that
    if let Some(romfs_base) = romfs_base {
        let Some(x) = find_file_in_romfs_root(Path::new(romfs_base), file) else {
            cu::bail!("failed to find romfs file '{file}' in '{romfs_base}'");
        };
        cu::info!("found romfs file '{file}': {}", x.try_to_rel().display());
        return Ok(x);
    }
    cu::debug!("--romfs option not given, trying to infer location of '{file}'");
    // otherwise, try to search for romfs directory
    let base_romfs = base.join("romfs");
    if base_romfs.is_dir() {
        cu::debug!("trying to find location of '{file}' in {}", base_romfs.display());
        if let Some(path) = find_file_in_romfs_root(&base_romfs, file) {
            cu::info!("found romfs file '{file}': {}", path.try_to_rel().display());
            return Ok(path);
        }
    }
    // try parent of base
    let parent = base.parent_abs()?;
    let parent_romfs = parent.join("romfs");
    if parent_romfs.is_dir() {
        cu::debug!("trying to find location of '{file}' in {}", parent_romfs.display());
        if let Some(path) = find_file_in_romfs_root(&parent_romfs, file) {
            cu::info!("found romfs file '{file}': {}", path.try_to_rel().display());
            return Ok(path);
        }
    }
    cu::bail!("could not find romfs file '{file}'");
}

fn find_file_in_romfs_root(root: &Path, file: &str) -> Option<PathBuf> {
    let mut path = root.join(file);
    if path.is_file() {
        return Some(path);
    }
    // try sbyml/byml interchange
    if file.ends_with(".sbyml") {
        path.set_extension("byml");
        if path.is_file() {
            return Some(path);
        }
    } else if file.ends_with(".byml") {
        path.set_extension("sbyml");
        if path.is_file() {
            return Some(path);
        }
    }

    None
}

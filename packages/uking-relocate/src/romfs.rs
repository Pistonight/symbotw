use std::path::{Path, PathBuf};

use anyhow::bail;

use blueflame::env::DataId;
use blueflame::program::ProgramData;

pub struct Romfs {
    /// Path to Actor/ActorInfo.product.sbyml (or .byml)
    pub actor_info: PathBuf,
}

impl Romfs {
    pub fn find_paths(
        sdk_path: impl AsRef<Path>,
        romfs_path: Option<&Path>,
    ) -> anyhow::Result<Self> {
        let sdk_path = sdk_path.as_ref();
        let Some(exefs_dir) = sdk_path.parent() else {
            bail!("failed to find romfs directory");
        };
        let Some(actor_info) =
            find_romfs_file(exefs_dir, romfs_path, "Actor/ActorInfo.product.sbyml")
        else {
            bail!("failed to find Actor/ActorInfo.product.sbyml in romfs");
        };
        println!("-- [romfs] found ActorInfo.product.sbyml");
        Ok(Self { actor_info })
    }

    pub fn load_actor_info_data(&self) -> anyhow::Result<ProgramData> {
        println!("-- [romfs] loading ActorInfo.product.sbyml");
        let bytes = std::fs::read(&self.actor_info)?;
        let decompressed_bytes = roead::yaz0::decompress_if(&bytes);

        let actor_info = ProgramData::new(DataId::ActorInfoByml, decompressed_bytes.to_vec());
        Ok(actor_info)
    }
}

fn find_romfs_file(base: &Path, romfs_base: Option<&Path>, file: &str) -> Option<PathBuf> {
    if let Some(romfs_base) = romfs_base {
        return find_file_in_romfs_root(romfs_base, file);
    }
    let base_romfs = base.join("romfs");
    if base_romfs.is_dir() {
        if let Some(path) = find_file_in_romfs_root(&base_romfs, file) {
            return Some(path);
        }
    }
    // try parent of base
    let parent = base.parent()?;
    let parent_romfs = parent.join("romfs");
    if parent_romfs.is_dir() {
        if let Some(path) = find_file_in_romfs_root(&parent_romfs, file) {
            return Some(path);
        }
    }
    None
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

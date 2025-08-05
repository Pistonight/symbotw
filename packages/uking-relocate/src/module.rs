use std::path::Path;

use blueflame::env::GameVer;
use cu::pre::*;

pub struct ModuleData {
    pub rtld: Vec<u8>,
    pub main: Vec<u8>,
    pub subsdk0: Vec<u8>,
    pub sdk: Vec<u8>,
    pub info: Modules,
}

impl ModuleData {
    pub fn load(path: &str) -> cu::Result<Self> {
        if !path.contains("sdk") {
            cu::bail!("the input file must contain 'sdk' in its name")
        }
        cu::info!("loading sdk module from {path}");
        // note that we cannot use any integrity checks here,
        // as the ELF files could be different depending on how
        // it is decompressed and converted from NSO
        let sdk_data = cu::fs::read(path)?;
        let has_150 = memchr::memmem::find(&sdk_data, b"sdk_version: 4.4.0").is_some();
        let has_160 = memchr::memmem::find(&sdk_data, b"sdk_version: 7.3.2").is_some();
        let info = match (has_150, has_160) {
            (true, false) => {
                cu::info!("sdk version matches 1.5.0");
                Modules::new_150()
            }
            (false, true) => {
                cu::info!("sdk version matches 1.6.0");
                Modules::new_160()
            }
            // TODO: 1.8
            _ => cu::bail!("the input files does not match a supported version (only 1.5.0 and 1.6.0 are supported right now)"),
        };

        let path = Path::new(path);
        let file_name = path.file_name_str()?;
        let directory = path.parent_abs().context("cannot get exefs directory")?;

        let rtld_path = directory.join(file_name.replace("sdk", "rtld"));
        cu::info!("inferred path for rtld    : {}", rtld_path.try_to_rel().display());
        let rtld_data = cu::fs::read(&rtld_path)?;

        let main_path = directory.join(file_name.replace("sdk", "main"));
        cu::info!("inferred path for main    : {}", main_path.try_to_rel().display());
        let main_data = cu::fs::read(&main_path)?;

        let subsdk0_path = directory.join(file_name.replace("sdk", "subsdk0"));
        cu::info!("inferred path for subsdk0 : {}", subsdk0_path.try_to_rel().display());
        let subsdk0_data = cu::fs::read(&subsdk0_path)?;

        let data = Self {
            rtld: rtld_data,
            main: main_data,
            subsdk0: subsdk0_data,
            sdk: sdk_data,
            info,
        };

        Ok(data)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Modules {
    pub ver: GameVer,
    pub rtld: ModuleInfo,
    pub main: ModuleInfo,
    pub subsdk0: ModuleInfo,
    pub sdk: ModuleInfo,
}

impl Modules {
    pub fn new_150() -> Self {
        Self {
            ver: GameVer::X150,
            rtld: ModuleInfo {
                start: 0x0,
                text_end: 0x2000,
                end: 0x4000,
            },
            main: ModuleInfo {
                start: 0x4000,
                text_end: 0x1807000,
                end: 0x26af000,
            },
            subsdk0: ModuleInfo {
                start: 0x26af000,
                text_end: 0x29ba000,
                end: 0x2d95000,
            },
            sdk: ModuleInfo {
                start: 0x2d95000,
                text_end: 0x31a4000,
                end: 0x381e000,
            },
        }
    }
    pub fn new_160() -> Self {
        Self {
            ver: GameVer::X160,
            rtld: ModuleInfo {
                start: 0x0,
                text_end: 0x2000,
                end: 0x4000,
            },
            main: ModuleInfo {
                start: 0x4000,
                text_end: 0x212e000,
                end: 0x2d6a000,
            },
            subsdk0: ModuleInfo {
                start: 0x2d6a000,
                text_end: 0x30de000,
                end: 0x3487000,
            },
            sdk: ModuleInfo {
                start: 0x3487000,
                text_end: 0x39b5000,
                end: 0x415b000,
            },
        }
    }

    /// Given an offset relative to module start, get the offset relative to
    /// program start.
    ///
    /// If the offset is after the end of the module, return the end of the module,
    pub fn to_program_offset(&self, module: ModuleType, offset: u32) -> u32 {
        let info = match module {
            ModuleType::None => &self.rtld,
            ModuleType::Main => &self.main,
            ModuleType::Subsdk0 => &self.subsdk0,
            ModuleType::Sdk => &self.sdk,
        };
        let addr = info.start + offset;
        addr.min(info.end)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModuleInfo {
    /// load offset of the module (must be page aligned)
    pub start: u32,
    /// end offset of the module's .text section (must be page aligned)
    ///
    /// This is used to verify we loaded the module correctly
    pub text_end: u32,
    /// end offset of the module (must be page aligned)
    pub end: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModuleType {
    /// program start (i.e. rtld)
    None,
    /// main module aka uking
    Main,
    /// subsdk0 aka multimedia
    Subsdk0,
    /// sdk aka nnSdk
    Sdk,
}

impl std::fmt::Display for ModuleType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ModuleType::None => write!(f, "rtld"),
            ModuleType::Main => write!(f, "main"),
            ModuleType::Subsdk0 => write!(f, "subsdk0"),
            ModuleType::Sdk => write!(f, "sdk"),
        }
    }
}

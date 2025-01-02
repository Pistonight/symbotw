use std::path::Path;

use anyhow::{anyhow, bail};


pub struct ModuleData {
    pub rtld: Vec<u8>,
    pub main: Vec<u8>,
    pub subsdk0: Vec<u8>,
    pub sdk: Vec<u8>,
}

impl ModuleData {
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        let main_data = std::fs::read(path)?;
        let main_digest = sha256::digest(&main_data);
        let mut data = Self {
            rtld: vec![], main: main_data, subsdk0: vec![], sdk: vec![],
        };

        let info_150 = Modules::new_1_5_0();
        if info_150.main.sha256 == main_digest {
            println!("found 1.5.0 main: {}", path.display());
            data.load_other_modules_with_info(path, &info_150)?;
            return Ok(data);
        }

        let info_160 = Modules::new_1_6_0();
        if info_160.main.sha256 == main_digest {
            println!("found 1.6.0 main: {}", path.display());
            data.load_other_modules_with_info(path, &info_160)?;
            return Ok(data);
        }

        bail!("the input files does not match a known version of the game")
    }

    fn load_other_modules_with_info(&mut self, path: &Path, info: &Modules) -> anyhow::Result<()> {
        let dir = path.parent().ok_or_else(|| anyhow!("no parent directory"))?;
        let mut found_count = 0;
        for entry in dir.read_dir()? {
            if found_count == 3 {
                break;
            }
            let entry = entry?;
            if !entry.file_type()?.is_file() {
                continue;
            }
            let data = std::fs::read(entry.path())?;
            let digest = sha256::digest(&data);
            if self.rtld.is_empty() {
                if digest == info.rtld.sha256 {
                    self.rtld = data;
                    println!("found rtld: {}", entry.path().display());
                    found_count += 1;
                    continue;
                }
            } 
            if self.subsdk0.is_empty() {
                if digest == info.subsdk0.sha256 {
                    self.subsdk0 = data;
                    println!("found subsdk0: {}", entry.path().display());
                    found_count += 1;
                    continue;
                }
            } 
            if self.sdk.is_empty() {
                if digest == info.sdk.sha256 {
                    self.sdk = data;
                    println!("found sdk: {}", entry.path().display());
                    found_count += 1;
                    continue;
                }
            }
        }

        if found_count != 3 {
            let mut missing = vec![];
            if self.rtld.is_empty() {
                missing.push("rtld");
            }
            if self.subsdk0.is_empty() {
                missing.push("subsdk0");
            }
            if self.sdk.is_empty() {
                missing.push("sdk");
            }
            bail!("cannot find all of the game's input files: missing {:?}", missing);
        }

        Ok(())
    }
}

pub struct Modules {
    pub rtld: ModuleInfo,
    pub main: ModuleInfo,
    pub subsdk0: ModuleInfo,
    pub sdk: ModuleInfo,
}

impl Modules {
    pub fn new_1_5_0() -> Self {
        Self {
            rtld: ModuleInfo {
                sha256: "2293e1cb19e3ebdf5265e2351e89a4cfa8fa274baf14d8fd83b469d74eea09fb",
                name: "nnrtld",
                start: 0x0,
                end: 0x2000,
            },
            main: ModuleInfo {
                sha256: "728b7d01c464902683982c6979beb1cd57e74c2fe419ab3ad5d74bb70e3f973c",
                name: "U-King.nss",
                start: 0x4000,
                end: 0x1807000,
            },
            subsdk0: ModuleInfo {
                sha256: "3ab0a85544ebd8c13e9c074cb72f7e18fc504d9f70cc8e9a991479221b55807c",
                name: "multimedia",
                start: 0x26af000,
                end: 0x29ba000,
            },
            sdk: ModuleInfo {
                sha256: "7afc35303a9a3e0036f5d0a9c6f8e2089404ba7403e4f9694bce08537002ad16",
                name: "nnSdk",
                start: 0x2d95000,
                end: 0x31a4000,
            },
        }
    }
    pub fn new_1_6_0() -> Self {
        Self {
            rtld: ModuleInfo {
                sha256: "bcda21e398d442db2a2bc024c1ecd3e0eb056a7882ecc32935fa2271d74c9bae",
                name: "nnrtld",
                start: 0x0,
                end: 0x2000,
            },
            main: ModuleInfo {
                sha256: "a6d764570f1886ce63b0c6cd04f82aec91755c6a84554dab27138b6f5b63ae92",
                name: "U-King.nss",
                start: 0x4000,
                end: 0x212e000,
            },
            subsdk0: ModuleInfo {
                sha256: "57e0d66b02f95b5cd2d27917f626480f828ea543d1552a11b13b29274c8b40a0",
                name: "multimedia",
                start: 0x2d6a000,
                end: 0x30de000,
            },
            sdk: ModuleInfo {
                sha256: "cbb9459c1aae22fee8d40719c215c449b94718e9833b127c10fe751c19e0c086",
                name: "nnSdk",
                start: 0x3487000,
                end: 0x39b5000,
            },
        }
    }
}

pub struct ModuleInfo {
    /// sha256 hash of the module
    pub sha256: &'static str,
    /// name of the module
    pub name: &'static str,
    /// load offset of the module (must be page aligned)
    pub start: u32,
    /// end offset of the module (must be page aligned)
    pub end: u32,
}

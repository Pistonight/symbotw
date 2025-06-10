use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, HashMap};

use anyhow::{anyhow, bail};
use derive_more::Deref;

use elf::abi::{STB_LOCAL, STB_WEAK, STV_HIDDEN, STV_INTERNAL, STV_PROTECTED};
use elf::endian::LittleEndian;
use elf::parse::{ParsingIterator, ParsingTable};
use elf::relocation::Rela;
use elf::segment::ProgramHeader;
use elf::ElfBytes;

use crate::module::ModuleType;

#[derive(Deref)]
pub struct ElfWrapper<'data> {
    #[deref]
    elf: ElfBytes<'data, LittleEndian>,
    /// program segments
    pub segments: ParsingTable<'data, LittleEndian, ProgramHeader>,
}

impl<'data> ElfWrapper<'data> {
    pub fn try_parse(data: &'data [u8]) -> anyhow::Result<Self> {
        let elf = ElfBytes::minimal_parse(data)?;
        let segments = elf
            .segments()
            .ok_or_else(|| anyhow!("unexpected empty program header table"))?;
        Ok(Self { elf, segments })
    }

    /// Load the defined dynamic symbols from this ELF file and store them by name
    ///
    /// start is the absolute address of this module
    ///
    /// Return how many symbols are loaded
    pub fn load_dynamic_symbols(
        &self,
        module: ModuleType,
        start: u64,
        table: &mut BTreeMap<String, SymbolValue>,
    ) -> anyhow::Result<u32> {
        let (dynsyms, strtab) = self
            .dynamic_symbol_table()?
            .ok_or_else(|| anyhow!("missing dynamic symbol table"))?;
        let mut count = 0;
        for sym in dynsyms {
            if sym.is_undefined() {
                // no need to load undefined symbols - as they are defined somewhere else
                continue;
            }
            if sym.st_name == 0 {
                // symbol doesn't have a name
                continue;
            }
            let name = strtab.get(sym.st_name as usize)?;
            let bind_type = sym.st_bind();
            let visibility = sym.st_vis();
            match visibility {
                STV_HIDDEN | STV_INTERNAL => {
                    // skip hidden/internal symbols
                    continue;
                }
                _ => {}
            };
            if bind_type == STB_LOCAL {
                // skip local symbols
                continue;
            }
            let value = SymbolValue {
                address: sym.st_value + start,
                weak: bind_type == STB_WEAK,
                protected: visibility == STV_PROTECTED,
            };
            match table.entry(name.to_string()) {
                Entry::Vacant(entry) => {
                    entry.insert(value);
                }
                Entry::Occupied(mut entry) => {
                    if entry.get().weak {
                        // linker can choose one arbitrarily if new one is also weak
                        entry.insert(value);
                    } else if !value.weak {
                        // if both are strong, it's an error
                        bail!("duplicate symbol in {}: {}", module, name);
                    }
                }
            }
            count += 1;
        }
        Ok(count)
    }

    // BOTW only has .rela.dyn and .rela.plt sections, not .rel

    /// Get the iterator for the .rela.dyn section
    pub fn rela_dyn(&self) -> anyhow::Result<ParsingIterator<'data, LittleEndian, Rela>> {
        let rela_dyn = self
            .section_header_by_name(".rela.dyn")?
            .ok_or_else(|| anyhow!("missing .rela.dyn section"))?;
        let rela_dyn = self.section_data_as_relas(&rela_dyn)?;
        Ok(rela_dyn)
    }

    /// Get the iterator for the .rela.plt section
    pub fn rela_plt(&self) -> anyhow::Result<ParsingIterator<'data, LittleEndian, Rela>> {
        let rela_plt = self
            .section_header_by_name(".rela.plt")?
            .ok_or_else(|| anyhow!("missing .rela.plt section"))?;
        let rela_plt = self.section_data_as_relas(&rela_plt)?;
        Ok(rela_plt)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct DynamicSymbolTables {
    pub rtld: BTreeMap<String, SymbolValue>,
    pub main: BTreeMap<String, SymbolValue>,
    pub subsdk0: BTreeMap<String, SymbolValue>,
    pub sdk: BTreeMap<String, SymbolValue>,

    // these are provided by magic
    pub magic: HashMap<String, SymbolValue>,
}

impl DynamicSymbolTables {
    pub fn new(start: u64, size: u32) -> Self {
        let mut magic = HashMap::new();
        // these are guesses... we can probably verify
        // but it's not really important
        magic.insert(
            "__EX_start".to_string(),
            SymbolValue {
                address: start,
                weak: false,
                protected: false,
            },
        );
        magic.insert(
            "__EX_end".to_string(),
            SymbolValue {
                address: start + size as u64,
                weak: false,
                protected: false,
            },
        );
        Self {
            rtld: BTreeMap::new(),
            main: BTreeMap::new(),
            subsdk0: BTreeMap::new(),
            sdk: BTreeMap::new(),
            magic,
        }
    }
    /// Get the absolute physical address of a dynamic symbol
    ///
    /// module is the module that is trying to resolve the symbol
    pub fn resolve(&self, _module: ModuleType, name: &str) -> anyhow::Result<u64> {
        if let Some(symbol) = self.magic.get(name) {
            return Ok(symbol.address);
        }
        let mut results = vec![];
        if let Some(symbol) = self.rtld.get(name) {
            results.push((ModuleType::None, symbol));
        }
        if let Some(symbol) = self.main.get(name) {
            results.push((ModuleType::Main, symbol));
        }
        if let Some(symbol) = self.subsdk0.get(name) {
            results.push((ModuleType::Subsdk0, symbol));
        }
        if let Some(symbol) = self.sdk.get(name) {
            results.push((ModuleType::Sdk, symbol));
        }
        if results.is_empty() {
            bail!("cannot resolve dynamic symbol: {}", name);
        }
        if results.len() == 1 {
            return Ok(results[0].1.address);
        }
        // if all of the symbols are weak, choose one arbitrarily
        if results.iter().all(|(_, symbol)| symbol.weak) {
            return Ok(results[0].1.address);
        }
        // if one is strong, pick that one
        let mut strong_sym = None;
        for (_, symbol) in &results {
            if !symbol.weak {
                if strong_sym.is_some() {
                    bail!("conflicting strong symbol: {}", name);
                }
                strong_sym = Some(symbol);
            }
        }
        if let Some(symbol) = strong_sym {
            return Ok(symbol.address);
        }
        println!("{results:?}");
        // found more than one symbol, does it even happen?
        bail!("ambiguous symbol: {name}");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SymbolValue {
    /// The absolute physical address of the symbol
    pub address: u64,
    /// If the symbol is weak
    pub weak: bool,
    /// If the symbol is protected, meaning it will resolve to
    /// the definition in the same module
    pub protected: bool,
}

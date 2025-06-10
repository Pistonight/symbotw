use std::collections::BTreeSet;

use anyhow::{anyhow, bail};
use elf::abi::{
    PT_LOAD, R_AARCH64_ABS64, R_AARCH64_GLOB_DAT, R_AARCH64_JUMP_SLOT, R_AARCH64_RELATIVE,
};

use blueflame::program;

use crate::{
    cli::RegionArg,
    elf::{DynamicSymbolTables, ElfWrapper},
    module::{ModuleData, ModuleInfo, ModuleType, Modules},
};

/// The loaded program memory layout
pub struct Memory {
    info: Modules,
    start: u64,
    pub regions: Vec<Region>,
    loaded_size: u32,
}

impl Memory {
    /// Load the modules into memory and perform relocation/dynamic linking
    pub fn load(start: u64, module_data: &ModuleData) -> anyhow::Result<Self> {
        let mut mem = Self {
            info: module_data.info.clone(),
            start,
            regions: Vec::new(),
            loaded_size: 0,
        };

        println!("-- [exefs] parsing ELF files...");

        let rtld_elf = ElfWrapper::try_parse(&module_data.rtld)?;
        let main_elf = ElfWrapper::try_parse(&module_data.main)?;
        let subsdk0_elf = ElfWrapper::try_parse(&module_data.subsdk0)?;
        let sdk_elf = ElfWrapper::try_parse(&module_data.sdk)?;

        println!("-- [exefs] loading modules into memory...");

        println!();
        println!("SEGMENT START      FILE_SIZE  MEM_SIZE");

        mem.load_module(ModuleType::None, &rtld_elf, &module_data.info.rtld)?;
        mem.load_module(ModuleType::Main, &main_elf, &module_data.info.main)?;
        mem.load_module(ModuleType::Subsdk0, &subsdk0_elf, &module_data.info.subsdk0)?;
        mem.load_module(ModuleType::Sdk, &sdk_elf, &module_data.info.sdk)?;

        mem.loaded_size = module_data.info.sdk.end;

        println!("-- [exefs] loading dynamic symbols...");
        let mut dynamic_symbols = DynamicSymbolTables::new(start, mem.loaded_size);
        let count = rtld_elf.load_dynamic_symbols(
            ModuleType::None,
            start + module_data.info.rtld.start as u64,
            &mut dynamic_symbols.rtld,
        )?;
        println!();
        println!("MODULE   DYNAMIC SYMBOLS");
        println!("rtld     {count}");
        let count = main_elf.load_dynamic_symbols(
            ModuleType::Main,
            start + module_data.info.main.start as u64,
            &mut dynamic_symbols.main,
        )?;
        println!("main     {count}");
        let count = subsdk0_elf.load_dynamic_symbols(
            ModuleType::Subsdk0,
            start + module_data.info.subsdk0.start as u64,
            &mut dynamic_symbols.subsdk0,
        )?;
        println!("subsdk0  {count}");
        let count = sdk_elf.load_dynamic_symbols(
            ModuleType::Sdk,
            start + module_data.info.sdk.start as u64,
            &mut dynamic_symbols.sdk,
        )?;
        println!("sdk      {count}");

        let mut count = 0;
        count += mem.relocate(
            ModuleType::None,
            &rtld_elf,
            &module_data.info.rtld,
            &dynamic_symbols,
        )?;
        count += mem.relocate(
            ModuleType::Main,
            &main_elf,
            &module_data.info.main,
            &dynamic_symbols,
        )?;
        count += mem.relocate(
            ModuleType::Subsdk0,
            &subsdk0_elf,
            &module_data.info.subsdk0,
            &dynamic_symbols,
        )?;
        count += mem.relocate(
            ModuleType::Sdk,
            &sdk_elf,
            &module_data.info.sdk,
            &dynamic_symbols,
        )?;
        println!("-- [exefs] applied {count} relocations across all modules",);

        Ok(mem)
    }

    /// Load the segments in the module into memory, without relocation
    fn load_module(
        &mut self,
        module: ModuleType,
        elf: &ElfWrapper,
        info: &ModuleInfo,
    ) -> anyhow::Result<()> {
        if self.loaded_size != info.start {
            bail!("unexpected loaded size mismatch for {}", module);
        }
        let mut segment_start = info.start;
        for ph in elf.segments {
            if ph.p_type == PT_LOAD {
                if ph.p_vaddr != ph.p_paddr {
                    bail!("unexpected p_vaddr != p_paddr");
                }
                if ph.p_vaddr != (segment_start - info.start) as u64 {
                    bail!(
                        "unexpected p_vaddr != start ({} != {})",
                        ph.p_vaddr,
                        segment_start - info.start
                    );
                }
                let segment_data = elf.segment_data(&ph)?;
                let permission = ph.p_flags;
                let region = Region::allocate(
                    module,
                    segment_start,
                    permission,
                    segment_data,
                    ph.p_memsz as u32,
                );
                let size = region.get_byte_len();
                println!(
                    "{:8}0x{:08x} 0x{:08x} 0x{:08x}  {}",
                    module.to_string(),
                    segment_start,
                    ph.p_memsz,
                    size,
                    perm_str(permission)
                );
                self.regions.push(region);
                segment_start += size;
                self.loaded_size = segment_start;
                if permission == 5 {
                    // RX
                    if segment_start != info.text_end {
                        bail!("unexpected text end mismatch for {module}");
                    }
                }
            }
        }
        if segment_start != info.end {
            bail!(
                "unexpected end mismatch for {}, expected 0x{:08x}, actual 0x{:08x}",
                module,
                info.end,
                segment_start
            );
        }

        Ok(())
    }

    /// Apply relocation to the module in memory, return how many relocations were applied
    fn relocate(
        &mut self,
        module: ModuleType,
        elf: &ElfWrapper,
        info: &ModuleInfo,
        dynamic: &DynamicSymbolTables,
    ) -> anyhow::Result<u32> {
        println!("-- [exefs] applying relocation to {module}");

        let mut module_regions = self
            .regions
            .iter_mut()
            .filter(|r| r.module == module)
            .collect::<Vec<_>>();
        let (symbols, strtab) = elf
            .dynamic_symbol_table()?
            .ok_or_else(|| anyhow!("missing dynamic symbol table"))?;
        let mut unresolved_global_data = BTreeSet::new();
        let mut unresolved_global_plt = BTreeSet::new();

        let mut count = 0;

        // dyn doesn't have JUMP_SLOT
        for rela in elf.rela_dyn()? {
            match rela.r_type {
                R_AARCH64_ABS64 => {
                    // maybe external functions in vtable?
                    if rela.r_sym == 0 {
                        bail!(
                            "unexpected empty r_sym in .rela.dyn: 0x{:08x}",
                            rela.r_offset
                        );
                    }
                    if rela.r_addend < 0 {
                        // BOTW only has positive r_addend
                        bail!(
                            "unexpected negative r_addend in .rela.dyn: {}",
                            rela.r_addend
                        );
                    }
                    let symbol = symbols.get(rela.r_sym as usize)?;
                    let symbol_name = strtab.get(symbol.st_name as usize)?;
                    let address = dynamic.resolve(module, symbol_name)? + rela.r_addend as u64;
                    Self::write_relocation(&mut module_regions, rela.r_offset as u32, address)?;
                    count += 1;
                }
                R_AARCH64_GLOB_DAT => {
                    // external data symbols
                    if rela.r_sym == 0 {
                        bail!(
                            "unexpected empty r_sym in .rela.dyn: 0x{:08x}",
                            rela.r_offset
                        );
                    }
                    if rela.r_addend != 0 {
                        bail!(
                            "unexpected r_addend in .rela.dyn for GLOB_DAT: {}",
                            rela.r_addend
                        );
                    }
                    let symbol = symbols.get(rela.r_sym as usize)?;
                    let symbol_name = strtab.get(symbol.st_name as usize)?;
                    let address = match dynamic.resolve(module, symbol_name) {
                        Ok(address) => address,
                        Err(_) => {
                            // given it's global variable, it might be OK, if we are not touching
                            // it
                            unresolved_global_data.insert(symbol_name.to_string());
                            0
                        }
                    };
                    Self::write_relocation(&mut module_regions, rela.r_offset as u32, address)?;
                    count += 1;
                }
                R_AARCH64_RELATIVE => {
                    // these are things like vtables in .data
                    if rela.r_sym != 0 {
                        bail!(
                            "unexpected r_sym in {}.rela.dyn: 0x{:08x}",
                            module,
                            rela.r_sym
                        );
                    }
                    let offset = rela.r_offset as u32;
                    if rela.r_addend < 0 {
                        bail!(
                            "unexpected r_addend in {}.rela.dyn: 0x{:08x}",
                            module,
                            rela.r_addend
                        );
                    }
                    let value = info.start as u64 + rela.r_addend as u64 + self.start;

                    Self::write_relocation(&mut module_regions, offset, value)?;
                    count += 1;
                }
                _ => {
                    bail!("unexpected relocation type in .rela.dyn: {}", rela.r_type);
                }
            }
        }

        // plt only has JUMP_SLOT
        for rela in elf.rela_plt()? {
            if rela.r_addend != 0 {
                bail!("unexpected r_addend in .rela.plt: {}", rela.r_addend);
            }
            match rela.r_type {
                R_AARCH64_JUMP_SLOT => {
                    if rela.r_sym == 0 {
                        bail!(
                            "unexpected empty r_sym in .rela.plt: 0x{:08x}",
                            rela.r_offset
                        );
                    }
                    if rela.r_addend != 0 {
                        bail!("unexpected r_addend in .rela.plt: 0x{:08x}", rela.r_addend);
                    }
                    let symbol = symbols.get(rela.r_sym as usize)?;
                    let symbol_name = strtab.get(symbol.st_name as usize)?;
                    let address = match dynamic.resolve(module, symbol_name) {
                        Ok(address) => address,
                        Err(_) => {
                            // only happens for SDK, which aren't called
                            // so it's probably fine?
                            unresolved_global_plt.insert(symbol_name.to_string());
                            0
                        }
                    };

                    // according to reference this should be creating PLT entry
                    // but in BOTW this seems to be creating .got.plt entry.
                    // These are just 1 pointer to the actual function
                    // the PLT entry is created statically to load GOT
                    Self::write_relocation(&mut module_regions, rela.r_offset as u32, address)?;
                    count += 1;
                }
                _ => {
                    bail!("unexpected relocation type in .rela.plt: {}", rela.r_type);
                }
            }
        }

        if !unresolved_global_data.is_empty() {
            println!(
                "WARNING - the following global variables are unresolved: {unresolved_global_data:?}",
            );
        }
        if !unresolved_global_plt.is_empty() {
            println!(
                "WARNING - the following GOT PLT entries are unresolved: {unresolved_global_data:?}",
            );
        }
        Ok(count)
    }

    /// Write the relocation value to offset in the region
    fn write_relocation(
        regions: &mut [&mut Region],
        offset: u32,
        value: u64,
    ) -> anyhow::Result<()> {
        // convert offset from relative to module start to relative to program start
        let offset = offset + regions[0].rel_start;
        for region in regions {
            if region.rel_start + region.get_byte_len() <= offset {
                // the offset is after this region
                continue;
            }
            if region.rel_start > offset {
                // the offset is before this region
                bail!("unexpected offset 0x{:08x} not in any region", offset);
            }
            region.write(offset, value);
            break;
        }

        Ok(())
    }

    pub fn add_program_segments(
        &self,
        regions: &[RegionArg],
        mut builder: program::BuilderPhase3,
    ) -> program::BuilderPhase3 {
        println!("-- [exefs] copying program memory...");
        let mut page_starts = BTreeSet::new();
        for region in regions {
            let region_start =
                align_down!(self.info.to_program_offset(region.module, region.start));
            let region_end = align_up!(self.info.to_program_offset(region.module, region.end));
            for page in (region_start / 0x1000)..(region_end / 0x1000) {
                page_starts.insert(page * 0x1000);
            }
        }
        // (start rel to program_start, num_pages)
        let mut page_regions = Vec::new();
        let mut page_starts_iter = page_starts.into_iter();
        if let Some(first_page) = page_starts_iter.next() {
            let mut current_page = first_page;
            let mut current_num_pages = 1;
            for page_start in page_starts_iter {
                if page_start == current_page + current_num_pages * 0x1000 {
                    current_num_pages += 1;
                } else {
                    page_regions.push((current_page, current_num_pages));
                    current_page = page_start;
                    current_num_pages = 1;
                }
            }
            page_regions.push((current_page, current_num_pages));
        } else {
            // include all of the program
            for region in &self.regions {
                page_regions.push((region.rel_start, region.get_num_pages()));
            }
        }

        let mut count = 0;
        for (rel_start, num_pages) in page_regions {
            builder = self.add_segments_in(rel_start, num_pages, &mut count, builder);
        }
        println!("-- [exefs] copied {count} segments");

        builder
    }

    /// Copy the region of memory into a vector of ProgramRegion
    ///
    /// Note that one input region may result into multiple output regions,
    /// since there are gaps between the regions in the memory.
    pub fn add_segments_in(
        &self,
        rel_start: u32,
        num_pages: u32,
        count: &mut u32,
        mut builder: program::BuilderPhase3,
    ) -> program::BuilderPhase3 {
        for region in &self.regions {
            if let Some((rel_start, data)) = region.get_overlapped(rel_start, num_pages) {
                builder = builder.add_segment(rel_start, data);
                *count += 1;
            }
        }
        builder
    }

    pub fn get_program_size(&self) -> u32 {
        self.loaded_size
    }
}

pub struct Region {
    module: ModuleType,
    /// relative start compared to the start of the memory
    pub rel_start: u32,
    pub permissions: u32,
    pages: Vec<Page>,
}

impl Region {
    /// Allocate a new region of memory.
    ///
    /// Just enough pages will be allocated for the data.
    pub fn allocate(
        module: ModuleType,
        start: u32,
        permissions: u32,
        data: &[u8],
        mem_size: u32,
    ) -> Self {
        let num_pages = if mem_size % 0x1000 == 0 {
            mem_size / 0x1000
        } else {
            mem_size / 0x1000 + 1
        } as usize;
        let mut pages = Vec::with_capacity(num_pages);
        for i in 0..num_pages {
            let from = i * 0x1000;
            pages.push(Page::copy(data, from));
        }
        Self {
            module,
            rel_start: start,
            permissions,
            pages,
        }
    }

    /// Get the number of pages in this region
    pub fn get_num_pages(&self) -> u32 {
        self.pages.len().try_into().unwrap()
    }

    /// Get the size of the region in bytes
    pub fn get_byte_len(&self) -> u32 {
        self.get_num_pages() * 0x1000
    }

    /// Write a value to offset to program memory
    pub fn write(&mut self, offset: u32, value: u64) {
        let rel_offset = offset - self.rel_start;
        let page_idx = (rel_offset / 0x1000) as usize;
        let page_offset = (rel_offset % 0x1000) as usize;
        self.pages[page_idx].data[page_offset..page_offset + 8]
            .copy_from_slice(&value.to_le_bytes());
    }

    /// Get memory in this region that overlaps with the given range
    /// Returns None if there is no overlap
    pub fn get_overlapped(&self, rel_start: u32, num_pages: u32) -> Option<(u32, Vec<u8>)> {
        let rel_end = rel_start + num_pages * 0x1000;
        if rel_end <= self.rel_start {
            // input range is before this region
            return None;
        }
        let self_rel_end = self.rel_start + self.get_num_pages() * 0x1000;
        if rel_start >= self_rel_end {
            // input range is after this region
            return None;
        }
        let rel_start = rel_start.max(self.rel_start);
        let rel_end = rel_end.min(self_rel_end);
        println!(
            "loading 0x{:08x}-0x{:08x} {} {}",
            rel_start,
            rel_end,
            perm_str(self.permissions),
            self.module
        );
        let page_start_idx = (rel_start - self.rel_start) / 0x1000;
        let page_end_idx = (rel_end - self.rel_start) / 0x1000;
        let mut out_mem = Vec::with_capacity((rel_end - rel_start) as usize);
        for i in page_start_idx..page_end_idx {
            out_mem.extend_from_slice(&self.pages[i as usize].data);
        }

        Some((rel_start, out_mem))
    }
}

pub struct Page {
    data: [u8; 0x1000],
}

impl Page {
    /// Create a page by copying a range from slice
    ///
    /// Uncovered parts of the page are zeroed out
    #[inline]
    pub fn copy(data: &[u8], from: usize) -> Self {
        let mut page = Page { data: [0; 0x1000] };
        if data.len() <= from {
            // this part of the page is not covered by the data (i.e. bss)
            return page;
        }
        let len = 0x1000.min(data.len() - from);
        page.data[0..len].copy_from_slice(&data[from..from + len]);
        page
    }
}

fn perm_str(permission: u32) -> String {
    let mut s = String::new();
    if permission & 4 != 0 {
        s.push('r');
    } else {
        s.push('-');
    }
    if permission & 2 != 0 {
        s.push('w');
    } else {
        s.push('-');
    }
    if permission & 1 != 0 {
        s.push('x');
    } else {
        s.push('-');
    }
    s
}

macro_rules! align_down {
    ($val:expr) => {
        $val / 0x1000 * 0x1000
    };
}
pub(crate) use align_down;

macro_rules! align_up {
    ($val:expr) => {{
        let val = $val;
        if val % 0x1000 == 0 {
            val
        } else {
            $val / 0x1000 * 0x1000 + 0x1000
        }
    }};
}
pub(crate) use align_up;

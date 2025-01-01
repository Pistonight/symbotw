#![allow(non_upper_case_globals)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use elf::ElfBytes;
use error_stack::{report, Report, Result, ResultExt};
use gimli::{
    DW_TAG_formal_parameter, DW_TAG_subprogram, DW_TAG_variable, DwAt, DwTag, DwarfFileType,
    EndianSlice,
};

use crate::parsed::{
    AddrType, AddressInfo, DataInfo, FuncInfo, TypeInfo, TypePrim, TypesStage0, TypesStage1,
    TypesStage6,
};
use common::ProgressPrinter;

mod entry_integer;
mod entry_name;
mod entry_subprogram;
mod entry_type;

mod read_namespace;
use read_namespace::*;

mod read_type;
use read_type::*;
mod read_struct_type;
use read_struct_type::*;
mod read_enum_type;
use read_enum_type::*;
mod read_union_type;
use read_union_type::*;
mod read_base_type;
use read_base_type::*;
mod read_subroutine_type;
use read_subroutine_type::*;

mod unit;
use unit::{bad, err_ctx, opt_ctx, UnitCtx};

pub type In<'i> = EndianSlice<'i, gimli::LittleEndian>;
pub type Unit<'i> = gimli::Unit<In<'i>>;
pub type UnitHeader<'i> = gimli::UnitHeader<In<'i>>;
pub type Tree<'i, 'a, 'u> = gimli::EntriesTree<'a, 'u, In<'i>>;
pub type Node<'i, 'a, 'u, 't> = gimli::EntriesTreeNode<'a, 'u, 't, In<'i>>;
pub type Dwarf<'i> = gimli::Dwarf<In<'i>>;
pub type UnitOffset = gimli::UnitOffset<usize>;
#[allow(clippy::upper_case_acronyms)]
pub type DIE<'i, 'a, 'u> = gimli::DebuggingInformationEntry<'a, 'u, In<'i>, usize>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Failed to read ELF file `{0}`")]
    ReadElf(String),
    #[error("Failed to parse ELF")]
    ParseElf,
    #[error("Failed to read header for section `{0}`")]
    SectionHeader(String),
    #[error("Failed to read unit header")]
    ReadUnitHeader,
    #[error("Failed to create unit")]
    CreateUnit,
    #[error("Failed to create unit abbreviations")]
    CreateUnitAbbrev,
    #[error("Failed to create unit, expecting a DebugInfoOffset")]
    CreateUnitOffset,

    #[error("Failed to create entry tree")]
    CreateTree,
    #[error("Failed to read root of entry tree")]
    ReadRoot,
    #[error("Failed to read child entry")]
    ReadChild,
    #[error("Expecting a child entry, but none found")]
    ExpectingChild,
    #[error("Failed to read entry in unit")]
    ReadEntry,
    #[error("Failed to read DIE attribute `{0}`")]
    ReadEntryAttr(DwAt),
    #[error("Expected DIE attribute `{0}`, but it's not found")]
    MissingEntryAttr(DwAt),
    #[error("Unexpected DIE attribute `{0}`")]
    UnexpectedEntryAttr(DwAt),
    #[error("DIE attribute `{0}` should have type `{1}`")]
    BadEntryAttrType(DwAt, &'static str),
    #[error("Unable to understand the combination of DIE attributes `{0}`")]
    BadEntryAttrTypes(String),

    #[error("Unexpected tag `{0}`")]
    UnexpectedTag(DwTag),
    #[error("Unexpected tag `{0}` for DIE, expecting tag for a type")]
    UnexpectedTagForType(DwTag),
    #[error("Unexpected base type for enumeration")]
    UnexpectedEnumType,
    #[error("Declaration cannot be anonymous")]
    UnexpectedAnonymousDeclaration,
    #[error("Unexpected virtual union")]
    UnexpectedVirtualUnion,
    #[error("Unexpected type name for DW_TAG_unspecified_type. You might want to extend the match statement")]
    UnspecifiedType,
    #[error("Failed to get namespace for DIE")]
    Namespace,
    #[error("Hole in vtable")]
    MissingVtableEntry,
    #[error("Conflicting entry in vtable")]
    ConflictingVtableEntry,
    #[error("Conflicting name for functions")]
    ConflictingName,
    #[error("Conflicting type for functions")]
    ConflictingInfo,
    #[error("DW_TAG_variable doesn't have a DW_AT_specification")]
    NoSpecificationInVariable,

    #[error("Failed to read string attribute")]
    AttrString,
    #[error("Failed to read address attribute")]
    AttrAddress,

    #[error("{0} at 0x{1:08x}")]
    Ctx(&'static str, usize),

    #[error("Failed to resolve type names")]
    ResolveName,
    #[error("Failed to resolve type sizes")]
    ResolveSize,
    #[error("Symbol looks like a decompiled symbol, but was not extracted from DWARF")]
    UnmatchedSymbol,
    #[error("Unexpected error when processing linkage name")]
    UnexpectedLinkageName,
}

macro_rules! process_units {
    ($units:ident, $desc:literal, $unit:ident, $root:ident, $block:block) => {{
        let progress = common::ProgressPrinter::new($units.len(), $desc);
        for (i, $unit) in $units.iter().enumerate() {
            progress.print(i, $unit.name);
            let mut tree = $unit.tree()?;
            let $root = $crate::dwarf::unit::err_ctx!($unit, Error::ReadRoot, tree.root())?;
            $block
        }
        progress.done();
    }};
}
pub(crate) use process_units;

/// Type and function info extracted from Dwarf
pub struct DwarfInfo {
    /// Map of symbol -> AddressInfo, including both data and function symbols
    pub address: BTreeMap<String, AddressInfo>,
    /// Resolved types
    pub types: TypesStage6,
}

pub fn extract(
    elf_path: &Path,
    uking_symbols: &mut BTreeMap<String, u64>,
    decompiled_functions: &BTreeSet<String>,
) -> Result<DwarfInfo, Error> {
    println!("Extracting DAWRF from ELF {}", elf_path.display());
    let uking_elf = std::fs::read(elf_path)
        .change_context_lazy(|| Error::ReadElf(elf_path.display().to_string()))?;
    let file = ElfBytes::<elf::endian::LittleEndian>::minimal_parse(&uking_elf)
        .change_context(Error::ParseElf)?;

    let mut dwarf = gimli::Dwarf::load(|section| {
        let header = file
            .section_header_by_name(section.name())
            .change_context_lazy(|| Error::SectionHeader(section.name().to_string()))?;
        match header {
            Some(header) => {
                let start = header.sh_offset as usize;
                let end = start + header.sh_size as usize;
                let slice = EndianSlice::new(&uking_elf[start..end], gimli::LittleEndian);
                Ok::<_, Report<Error>>(slice)
            }
            None => Ok(EndianSlice::new(&[], gimli::LittleEndian)),
        }
    })?;
    dwarf.file_type = DwarfFileType::Main;

    println!("Processing DWARF...");

    let debug_info = dwarf.debug_info;

    let mut iter = debug_info.units();
    let mut units = Vec::new();
    let mut progress = ProgressPrinter::new(0, "Discover compile units");
    while let Some(unit_header) = iter.next().change_context(Error::ReadUnitHeader)? {
        let unit_ctx = UnitCtx::new(unit_header, &dwarf)?;
        progress.set_total(units.len() + 1);
        progress.print(units.len(), unit_ctx.name);
        units.push(unit_ctx);
    }
    progress.done();

    // PASS 1 - namespace info
    let namespaces = read_namespace(&units)?;
    // PASS 2 - type info
    let mut types = {
        let mut types = TypesStage0::new();
        types.insert(usize::MAX, TypeInfo::Prim(TypePrim::Void));
        process_units!(units, "Register types", unit, root, {
            read_types(root, unit, &namespaces, &mut types)?;
        });
        types.into_stage1()
    };

    // PASS 3 - symbols
    let data_types = {
        let mut data_types = BTreeMap::new();
        let mut addr_to_name = BTreeMap::new();
        process_units!(units, "Process address symbols", unit, root, {
            pass3(
                root,
                unit,
                uking_symbols,
                &mut addr_to_name,
                &mut data_types,
                &mut types,
            )?;
        });
        let mut added_symbols = Vec::new();
        for (symbol, address) in uking_symbols.iter() {
            if symbol.starts_with("_Z") && decompiled_functions.contains(symbol) {
                // we already tried to coerce ctors and dtors, but it's possible they are still not
                // found.
                // For example, the CSV could have both D1 and D2, while the DWARF only has D2
                // it's probably fine to just coerce the type info
                let mut alt_names = Vec::new();
                if let Some(n) = replace_c_or_d_name(symbol, "D1", "D2") {
                    alt_names.push(n);
                }
                if let Some(n) = replace_c_or_d_name(symbol, "D1", "D0") {
                    alt_names.push(n);
                }
                if let Some(n) = replace_c_or_d_name(symbol, "D2", "D1") {
                    alt_names.push(n);
                }
                if let Some(n) = replace_c_or_d_name(symbol, "D2", "D0") {
                    alt_names.push(n);
                }
                if let Some(n) = replace_c_or_d_name(symbol, "D0", "D1") {
                    alt_names.push(n);
                }
                if let Some(n) = replace_c_or_d_name(symbol, "D0", "D2") {
                    alt_names.push(n);
                }
                if let Some(n) = replace_c_or_d_name(symbol, "C1", "C2") {
                    alt_names.push(n);
                }
                if let Some(n) = replace_c_or_d_name(symbol, "C2", "C1") {
                    alt_names.push(n);
                }
                let mut found = false;
                for n in alt_names {
                    if let Some(info) = data_types.get(&n) {
                        let mut info = info.clone();
                        info.name = symbol.clone();
                        info.uking_address = *address;
                        data_types.insert(symbol.clone(), info);
                        added_symbols.push(symbol.clone());
                        found = true;
                        break;
                    }
                }
                if !found {
                    let r = report!(Error::UnmatchedSymbol)
                        .attach_printable(format!("Symbol: {}", symbol))
                        .attach_printable(format!("Address: 0x{:016x}", address));
                    return Err(r);
                }
            }
        }
        for symbol in added_symbols {
            uking_symbols.remove(&symbol);
        }
        data_types
    };
    // Type GC
    let mut types = types.into_stage2();
    {
        let progress = ProgressPrinter::new(data_types.len(), "Garbage collect types");
        for (i, (name, info)) in data_types.iter().enumerate() {
            progress.print(i, name);
            info.mark_types(&mut types)
        }
        progress.done();
    }
    let types = types.sweep_into_stage3();
    let types = types.merge_into_stage4(&namespaces);
    let types = types
        .resolve_into_stage5()
        .change_context(Error::ResolveName)?;
    let types = types
        .resolve_into_stage6()
        .change_context(Error::ResolveSize)?;
    Ok(DwarfInfo {
        address: data_types,
        types,
    })
}

/////////////// PASS 3 ///////////////
// Create offset -> address symbol map (function and data)

fn pass3<'i>(
    node: Node<'i, '_, '_, '_>,
    unit: &UnitCtx<'_, 'i>,
    uking_symbols: &mut BTreeMap<String, u64>,
    elf_addr_to_name: &mut BTreeMap<u64, String>,
    data_type: &mut BTreeMap<String, AddressInfo>,
    types: &mut TypesStage1,
) -> Result<(), Error> {
    let entry = node.entry();
    match entry.tag() {
        DW_TAG_subprogram => {
            read_subprogram(
                entry,
                unit,
                uking_symbols,
                elf_addr_to_name,
                data_type,
                // None,
                types,
            )?;
        }
        DW_TAG_variable => {
            read_variable(entry, unit, uking_symbols, data_type, types)?;
        }
        _ => {}
    }
    unit.for_each_child(node, |child| {
        pass3(
            child,
            unit,
            uking_symbols,
            elf_addr_to_name,
            data_type,
            types,
        )
    })?;

    Ok(())
}

fn read_subprogram<'i>(
    entry: &DIE<'i, '_, '_>,
    unit: &UnitCtx<'_, 'i>,
    uking_symbols: &mut BTreeMap<String, u64>,
    elf_addr_to_name: &mut BTreeMap<u64, String>,
    data_type: &mut BTreeMap<String, AddressInfo>,
    types: &mut TypesStage1,
) -> Result<(), Error> {
    let offset = entry.offset();
    if let Some(linkage_name) = read_linkage_name(entry, unit)? {
        let addr = unit.get_entry_low_pc(entry)?;

        // produce AddressInfo
        // return type
        let ret_ty = read_function_type(entry, unit)?;
        let mut args = Vec::new();
        unit.for_each_child_entry(entry, |child| {
            let entry = child.entry();
            match entry.tag() {
                DW_TAG_formal_parameter => {
                    let name = unit.get_entry_name_optional(entry)?.map(|x| x.to_string());
                    let type_offset = unit
                        .get_entry_type_offset_optional(entry)?
                        .map(|x| unit.to_global_offset(x).into());
                    args.push((name, type_offset));
                }
                _ => {
                    // ignore subtypes
                }
            }
            Ok(())
        })?;
        let func_info = FuncInfo {
            ret_ty_offset: ret_ty.into(),
            args,
        };
        let addr_info = AddressInfo {
            uking_address: 0,
            name: linkage_name.to_string(),
            info: AddrType::Func(func_info),
        };
        // if the function has an address, check if it's conflicting
        if let Some(addr) = addr {
            // address 0 are not real
            if addr != 0 {
                if let Some(old_name) = elf_addr_to_name.insert(addr, linkage_name.to_string()) {
                    if old_name != linkage_name {
                        return bad!(unit, unit.to_global_offset(offset), Error::ConflictingName)
                            .attach_printable(format!("Function `{}`", linkage_name))
                            .attach_printable(format!(
                                "0x{:08x} is already assigned to `{}`",
                                addr, old_name
                            ));
                    }
                }
            }
        }
        if !data_type.contains_key(linkage_name) && !uking_symbols.contains_key(linkage_name) {
            // need to check that the compiler generated different names for ctor/dtors
            let mut alt_names = Vec::new();
            if linkage_name.contains("C1") {
                if let Some(alt_name) = try_get_alt_name(unit, entry, linkage_name, "C1", "C2")? {
                    alt_names.push(alt_name);
                }
            }
            if linkage_name.contains("C2") {
                if let Some(alt_name) = try_get_alt_name(unit, entry, linkage_name, "C2", "C1")? {
                    alt_names.push(alt_name);
                }
            }
            if linkage_name.contains("D0") {
                if let Some(alt_name) = try_get_alt_name(unit, entry, linkage_name, "D0", "D1")? {
                    alt_names.push(alt_name);
                }
                if let Some(alt_name) = try_get_alt_name(unit, entry, linkage_name, "D0", "D2")? {
                    alt_names.push(alt_name);
                }
            }
            if linkage_name.contains("D1") {
                // note: pick D2 first
                if let Some(alt_name) = try_get_alt_name(unit, entry, linkage_name, "D1", "D2")? {
                    alt_names.push(alt_name);
                }
                if let Some(alt_name) = try_get_alt_name(unit, entry, linkage_name, "D1", "D0")? {
                    alt_names.push(alt_name);
                }
            }
            if linkage_name.contains("D2") {
                // note: pick D1 first
                if let Some(alt_name) = try_get_alt_name(unit, entry, linkage_name, "D2", "D1")? {
                    alt_names.push(alt_name);
                }
                if let Some(alt_name) = try_get_alt_name(unit, entry, linkage_name, "D2", "D0")? {
                    alt_names.push(alt_name);
                }
            }
            for alt_name in alt_names {
                if try_add_or_merge_info(
                    unit,
                    &alt_name,
                    unit.to_global_offset(offset),
                    addr_info.clone(),
                    data_type,
                    types,
                    uking_symbols,
                )? {
                    break;
                }
            }
        } else {
            try_add_or_merge_info(
                unit,
                linkage_name,
                unit.to_global_offset(offset),
                addr_info,
                data_type,
                types,
                uking_symbols,
            )?;
        }
        return Ok(());
    }
    // ones that don't have name shouldn't matter
    Ok(())
}

fn read_linkage_name<'i>(
    entry: &DIE<'i, '_, '_>,
    unit: &UnitCtx<'_, 'i>,
) -> Result<Option<&'i str>, Error> {
    if let Some(linkage_name) = unit.get_entry_linkage_name(entry)? {
        return Ok(Some(linkage_name));
    }
    // no linkage_name, use other info to find the function name
    if let Some(abstract_origin) = unit.get_entry_abstract_origin(entry)? {
        let origin_entry = unit.entry_at(abstract_origin)?;
        return read_linkage_name(&origin_entry, unit);
    }
    if let Some(specification) = unit.get_entry_specification(entry)? {
        let origin_entry = unit.entry_at(specification)?;
        return read_linkage_name(&origin_entry, unit);
    }
    // ones that don't have name shouldn't matter
    Ok(None)
}
fn read_function_type<'i>(entry: &DIE<'i, '_, '_>, unit: &UnitCtx<'_, 'i>) -> Result<usize, Error> {
    if let Some(specification) = unit.get_entry_specification(entry)? {
        let origin_entry = unit.entry_at(specification)?;
        return read_function_type(&origin_entry, unit);
    }
    if let Some(abstract_origin) = unit.get_entry_abstract_origin(entry)? {
        let origin_entry = unit.entry_at(abstract_origin)?;
        return read_function_type(&origin_entry, unit);
    }
    unit.get_entry_type_global_offset(entry)
}

fn try_get_alt_name<'i>(
    unit: &UnitCtx<'_, 'i>,
    entry: &DIE<'i, '_, '_>,
    linkage_name: &str,
    original: &str, // C1, C2, D1, D2
    replace: &str,  // C2, C1, D2, D1
) -> Result<Option<String>, Error> {
    // do you have a name?
    let name = match unit.get_entry_name_optional(entry)? {
        Some(x) => x,
        None => {
            // do you have a specification?
            match unit.get_entry_specification(entry)? {
                Some(x) => {
                    return try_get_alt_name(
                        unit,
                        &unit.entry_at(x)?,
                        linkage_name,
                        original,
                        replace,
                    )
                }
                None => {
                    // do you have an abstract_origin?
                    match unit.get_entry_abstract_origin(entry)? {
                        Some(x) => {
                            return try_get_alt_name(
                                unit,
                                &unit.entry_at(x)?,
                                linkage_name,
                                original,
                                replace,
                            )
                        }
                        None => {
                            return Ok(None);
                        }
                    }
                }
            }
        }
    };
    let name = if let Some(stripped) = name.strip_prefix('~') {
        stripped
    } else {
        name
    };

    let name_count = name.matches(original).count();
    let linkage_count = linkage_name.matches(original).count();
    // generated name should have at least one more instance of original
    if linkage_count > name_count {
        let idx = linkage_name.rfind(original).unwrap();
        if !name.starts_with("operator") {
            // bro stops with the edge cases
            // the check below won't work for names with template in it
            // because linkage_name doesn't have < >
            // so we just use the first part, should still work (hopefully)
            let name = match name.find('<') {
                Some(x) => &name[..x],
                None => name,
            };
            // this guards against the case like:
            // linkage_name: foo::D1::Bar() (demangled)
            // name: Bar
            // then it's actually not a D1
            // This won't catch all cases, but hopefully people just don't name their functions D0 D1 D2
            match linkage_name.find(name) {
                None => {
                    return bad!(
                        unit,
                        unit.to_global_offset(entry.offset()),
                        Error::UnexpectedLinkageName
                    )
                    .attach_printable("linkage_name should include name")
                    .attach_printable(format!("linkage_name: {}", linkage_name))
                    .attach_printable(format!("name: {}", name));
                }
                Some(x) => {
                    if idx <= x {
                        return Ok(None);
                    }
                }
            };
        }
        let alt_name = replace_c_or_d_name(linkage_name, original, replace).unwrap();
        return Ok(Some(alt_name));
    }

    Ok(None)
}

fn replace_c_or_d_name(name: &str, original: &str, replace: &str) -> Option<String> {
    let idx = name.rfind(original)?;
    let s = format!(
        "{}{}{}",
        &name[..idx],
        replace,
        &name[idx + original.len()..]
    );
    Some(s)
}

fn try_add_or_merge_info(
    unit: &UnitCtx,
    linkage_name: &str,
    offset: usize,
    mut addr_info: AddressInfo,
    data_type: &mut BTreeMap<String, AddressInfo>,
    types: &mut TypesStage1,
    uking_symbols: &mut BTreeMap<String, u64>,
) -> Result<bool, Error> {
    // if there's some old info, merge it
    if let Some(old_info) = data_type.get_mut(linkage_name) {
        // replace name in case an alt name is used for ctor/dtor
        if addr_info.name != linkage_name {
            addr_info.name = linkage_name.to_string();
        }
        let check = old_info.check_and_merge(addr_info.clone(), types);
        err_ctx!(unit, offset, Error::ConflictingInfo, check)
            .attach_printable(format!("Function `{}`", linkage_name))
            .attach_printable(format!("Old Info: {}", old_info))
            .attach_printable(format!("New Info: {}", addr_info))?;
        return Ok(true);
    } else {
        if let Some(uking_address) = uking_symbols.remove(linkage_name) {
            // replace name in case an alt name is used for ctor/dtor
            if addr_info.name != linkage_name {
                addr_info.name = linkage_name.to_string();
            }
            addr_info.uking_address = uking_address;
            data_type.insert(linkage_name.to_string(), addr_info);
            return Ok(true);
        }
        // if there's no uking_address, it's not a symbol that we care
    }

    Ok(false)
}

fn read_variable<'i>(
    entry: &DIE<'i, '_, '_>,
    unit: &UnitCtx<'_, 'i>,
    uking_symbols: &mut BTreeMap<String, u64>,
    data_type: &mut BTreeMap<String, AddressInfo>,
    types: &mut TypesStage1,
) -> Result<(), Error> {
    if let Some(linkage_name) = unit.get_entry_linkage_name(entry)? {
        let ty_offset = {
            match unit.get_entry_type_offset_optional(entry)? {
                Some(x) => x,
                None => {
                    // try specification
                    let specification = unit.get_entry_specification(entry)?;
                    let specification = opt_ctx!(
                        unit,
                        unit.to_global_offset(entry.offset()),
                        Error::NoSpecificationInVariable,
                        specification
                    )?;
                    let spec_entry = unit.entry_at(specification)?;
                    unit.get_entry_type_offset(&spec_entry)?
                }
            }
        };

        let data_info = DataInfo {
            ty_offset: Some(unit.to_global_offset(ty_offset).into()),
        };
        let addr_info = AddressInfo {
            uking_address: 0,
            name: linkage_name.to_string(),
            info: AddrType::Data(data_info),
        };
        try_add_or_merge_info(
            unit,
            linkage_name,
            unit.to_global_offset(entry.offset()),
            addr_info,
            data_type,
            types,
            uking_symbols,
        )?;
    }

    // ignore no linkage name
    Ok(())
}

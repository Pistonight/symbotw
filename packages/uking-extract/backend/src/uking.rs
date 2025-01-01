use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use error_stack::{Result, ResultExt};

#[derive(Debug, thiserror::Error)]
pub enum UkingParseError {
    #[error("Failed to open file")]
    OpenFile,
    #[error("Failed to read file")]
    ReadFile,
    #[error("Missing function address")]
    MissingFuncAddr,
    #[error("Invalid function address: {0}")]
    InvalidFuncAddr(String),
    #[error("Missing function status")]
    MissingStatus,
    #[error("Missing function size")]
    MissingSize,
    #[error("Missing data address")]
    MissingDataAddr,
}

pub fn read_uking_functions(
    file: impl AsRef<Path>,
    out: &mut BTreeMap<String, u64>,
) -> Result<BTreeSet<String>, UkingParseError> {
    let path = file.as_ref().display().to_string();
    println!("Reading functions from {}", path);
    let file = File::open(file)
        .change_context(UkingParseError::OpenFile)
        .attach_printable_lazy(|| format!("Path: {}", path))?;
    let reader = BufReader::new(file);
    let mut decompiled = BTreeSet::new();
    for line in reader.lines() {
        let line = line.change_context(UkingParseError::ReadFile)?;
        if let Some((addr, name, is_ok)) = parse_uking_function(&line)? {
            if should_ignore_func(name) {
                continue;
            }
            if is_ok {
                decompiled.insert(name.to_string());
            }
            out.insert(name.to_string(), addr);
        }
    }
    Ok(decompiled)
}

fn parse_uking_function(line: &str) -> Result<Option<(u64, &str, bool)>, UkingParseError> {
    // examples:
    // 0x00000071000007a0,O,000032,_ZN4ksys3act8BaseProc11hasJobType_ENS0_7JobTypeE
    // 0x00000071000007c0,U,000156,ActorOption::m31
    if !line.starts_with("0x") {
        return Ok(None);
    }

    let mut parts = line.split(',');
    let addr = parts.next().ok_or(UkingParseError::MissingFuncAddr)?;
    let addr = u64::from_str_radix(&addr[2..], 16)
        .map_err(|_| UkingParseError::InvalidFuncAddr(addr.to_string()))?;

    let status = parts.next().ok_or(UkingParseError::MissingStatus)?;
    let decompiled = status == "O";
    let _ = parts.next().ok_or(UkingParseError::MissingSize)?;

    match parts.next() {
        Some(name) => Ok(Some((addr, name, decompiled))),
        None => {
            // if the name is missing, we just ignore this line
            Ok(None)
        }
    }
}

fn should_ignore_func(name: &str) -> bool {
    name.is_empty()
        || name.starts_with("sub_")
        || name.starts_with("nullsub_")
        || name.starts_with("j_")
}

pub fn read_uking_data(
    file: impl AsRef<Path>,
    out: &mut BTreeMap<String, u64>,
) -> Result<BTreeSet<String>, UkingParseError> {
    let path = file.as_ref().display().to_string();
    println!("Reading data symbols from {}", path);
    let file = File::open(file)
        .change_context(UkingParseError::OpenFile)
        .attach_printable_lazy(|| format!("Path: {}", path))?;
    let reader = BufReader::new(file);
    let mut names = BTreeSet::new();
    for line in reader.lines() {
        let line = line.change_context(UkingParseError::ReadFile)?;
        if let Some((addr, name)) = parse_uking_data(&line)? {
            out.insert(name.to_string(), addr);
            names.insert(name.to_string());
        }
    }
    Ok(names)
}

fn parse_uking_data(line: &str) -> Result<Option<(u64, &str)>, UkingParseError> {
    // examples:
    // 0x0000007102606910,_ZN4ksys12SystemTimers9sInstanceE
    // 0x0000007102650608,_ZN4ksys3act11BaseProcMgr9sInstanceE
    if !line.starts_with("0x") {
        return Ok(None);
    }

    let mut parts = line.split(',');
    let addr = parts.next().ok_or(UkingParseError::MissingDataAddr)?;
    let addr = u64::from_str_radix(&addr[2..], 16)
        .map_err(|_| UkingParseError::InvalidFuncAddr(addr.to_string()))?;

    match parts.next() {
        Some(name) => Ok(Some((addr, name))),
        None => {
            // if the name is missing, we just ignore this line
            Ok(None)
        }
    }
}

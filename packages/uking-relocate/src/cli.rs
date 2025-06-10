use anyhow::{anyhow, bail};
use clap::Parser;

use crate::{
    memory::{align_down, align_up},
    module::ModuleType,
};

#[derive(Debug, Parser)]
pub struct Cli {
    /// Path to the game's sdk module.
    ///
    /// This must be a decompressed ELF. This tool will automatically
    /// detect if you are using version 1.5.0 or 1.6.0, based
    /// on embedded sdk version
    ///
    /// Once the version is detected, "sdk" in the file name
    /// will be replaced with "main", "rtld" and "subsdk0" to
    /// find the other modules.
    pub sdk_elf: String,

    /// Path to the root of the romfs. Not all romfs files are required. See README
    ///
    /// If not specified, the program searches:
    /// - "romfs" directory in the same directory as the sdk elf
    /// - "romfs" directory in the parent directory of the sdk elf
    #[clap(long)]
    pub romfs: Option<String>,

    /// Path to the output file.
    #[clap(short, long, default_value = "program.bfi")]
    pub output: String,

    /// The physical start address of the program region.
    ///
    /// This is also the start of nnrtld. Address must be in hexadecimal and the leading 0x is optional and ignored.
    /// Additionally, the upper 24 bits and lower 20 bits must be zero.
    #[clap(short, long, value_parser(parse_u64))]
    pub start: u64,

    /// Regions of memory to keep in the program image, in the format
    /// of `[module]:start-end` (including the brackets). If empty, everything is kept.
    ///
    /// Module can be: [rtld (alias: nnrtld), main (alias: uking, u-king), subsdk0 (alias: multimedia), sdk (alias: nnsdk)],
    /// .nss postfixes are ignored. rtld is the same as not specifying a module.
    ///
    /// The start and end offsets are specified as relative offsets
    /// to --start or module if specified. The end offset is exclusive.
    /// They must be in hexadecimal and the leading 0x is optional and ignored
    ///
    /// Extra memory may be included if the inputs are not page aligned.
    #[clap(short, long, value_parser(parse_region))]
    pub regions: Vec<RegionArg>,
}

fn parse_region(arg: &str) -> anyhow::Result<RegionArg> {
    let (module, arg) = match arg.strip_prefix("[") {
        None => (ModuleType::None, arg),
        Some(rest) => {
            let rest = rest.trim_start();
            let mut parts = rest.splitn(2, "]:");
            let module_str = parts
                .next()
                .ok_or_else(|| anyhow!("invalid region syntax: cannot parse module"))?
                .trim();
            let rest = parts.next().ok_or_else(|| {
                anyhow!("invalid region syntax: missing address range after module")
            })?;
            if parts.next().is_some() {
                bail!("invalid region syntax: too many colons")
            }
            let module_str = module_str
                .strip_suffix(".nss")
                .unwrap_or(module_str)
                .to_ascii_lowercase();
            let module = match module_str.as_str() {
                "rtld" | "nnrtld" => ModuleType::None,
                "main" | "uking" | "u-king" => ModuleType::Main,
                "subsdk0" | "multimedia" => ModuleType::Subsdk0,
                "sdk" | "nnsdk" => ModuleType::Sdk,
                _ => {
                    bail!("invalid module: {}", module_str)
                }
            };
            (module, rest)
        }
    };
    let mut parts = arg.splitn(2, '-');
    let start = parts
        .next()
        .ok_or_else(|| anyhow!("invalid region syntax: missing start address"))?
        .trim();
    let end = parts
        .next()
        .ok_or_else(|| anyhow!("invalid region syntax: missing end address"))?
        .trim();
    if parts.next().is_some() {
        bail!("invalid region syntax: too many dashes")
    }
    // align to page boundary
    let start = align_down!(parse_u32(start)?);
    let end = align_up!(parse_u32(end)?);

    if start >= end {
        bail!("invalid region: start must be less than end")
    }

    Ok(RegionArg { module, start, end })
}

fn parse_u32(arg: &str) -> anyhow::Result<u32> {
    let arg = arg.trim_start_matches(['0', 'x', 'X']);
    if arg.is_empty() {
        return Ok(0);
    }
    Ok(u32::from_str_radix(arg, 16)?)
}

fn parse_u64(arg: &str) -> anyhow::Result<u64> {
    let arg = arg.trim_start_matches(['0', 'x', 'X']);
    if arg.is_empty() {
        return Ok(0);
    }
    Ok(u64::from_str_radix(arg, 16)?)
}

/// Region to keep in the output
#[derive(Debug, Clone)]
pub struct RegionArg {
    /// The module that the start and end are relative to
    pub module: ModuleType,
    /// The start offset
    pub start: u32,
    /// The end offset
    pub end: u32,
}

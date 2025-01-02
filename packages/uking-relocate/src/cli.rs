use clap::Parser;

#[derive(Debug, Parser)]
pub struct Cli {

    /// Path to the game's main executable.
    ///
    /// This must be a decompressed ELF. This tool will automatically
    /// detect if it's a 1.5.0 or 1.6.0 ELF, and searches for the other
    /// modules in the same directory.
    pub main_elf: String,
}


use std::process::ExitCode;

use clap::Parser;

mod module;
mod error;
mod cli;

use cli::Cli;
use module::ModuleData;

fn main() -> ExitCode {
    if let Err(e) = main_internal() {
        eprintln!("error: {:?}", e);
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

fn main_internal() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let data = ModuleData::load(cli.main_elf)?;

    Ok(())
}

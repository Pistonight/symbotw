use std::process::ExitCode;

use clap::Parser;
use error_stack::{report, ResultExt};

/// UKing Extract
///
/// Tool for extract type information from DWARF produced
/// by BOTW decomp project and generating import script for various frontends
#[derive(Debug, Clone, clap::Parser)]
pub struct CLI {
    #[clap(subcommand)]
    subcommand: Subcommand,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Subcommand {
    /// Extract data types from DWARF info from the botw decompile project
    Extract(backend::CLI),
    /// Generate a Python script to import extract data.
    Python(frontend::CLI),
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("backend error")]
    Backend,
    #[error("frontend error")]
    Frontend,
}

fn main() -> ExitCode {
    let CLI { subcommand } = CLI::parse();
    match subcommand {
        Subcommand::Extract(cli) => common::run(|| {
            let options = backend::Options::try_from(cli).change_context(Error::Backend)?;
            backend::extract(&options).change_context(Error::Backend)
        }),
        Subcommand::Python(cli) => common::run(|| {
            let options = frontend::Options::try_from(cli).change_context(Error::Frontend)?;
            if let Err(e) = frontend::run(&options) {
                // FIXME: consolidate anyhow and error-stack error handling
                eprintln!("{:#?}", e);
                return Err(report!(Error::Frontend));
            }
            Ok(())
        }),
    }
}

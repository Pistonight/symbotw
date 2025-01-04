use std::process::ExitCode;

use anyhow::bail;
use clap::Parser;

mod cli;
mod elf;
mod memory;
mod module;
mod singleton;

use cli::Cli;
use memory::Memory;
use module::ModuleData;
use uking_relocate_lib::{Env, ProgramBuilder};

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

    if cli.start & 0xFFFFFF00000FFFFF != 0 {
        bail!("invalid program start (see readme)");
    }

    // load the files
    let data = ModuleData::load(&cli.sdk_elf)?;
    let env = match (cli.dlc, data.info.is_1_6_0) {
        (false, false) => Env::X150,
        (false, true) => Env::X160,
        (true, false) => Env::X150DLC,
        (true, true) => Env::X160DLC,
    };

    let memory = Memory::load(cli.start, &data)?;

    let program = ProgramBuilder::new(env)
        .program(
            cli.start,
            memory.get_program_size(),
            memory.to_program_regions(&cli.regions),
        )
        .build();

    println!("-- packing the program...");
    let data = uking_relocate_lib::pack_blueflame(&program)?;
    println!("packed size: {} bytes", data.len());
    println!("-- verifying the pack...");
    let program2 = uking_relocate_lib::unpack_blueflame(&data)?;
    if program != program2 {
        bail!("the unpacked program does not match the original program");
    }

    println!("-- writing the output file...");

    std::fs::write(cli.output, data)?;

    println!("done!");

    Ok(())
}

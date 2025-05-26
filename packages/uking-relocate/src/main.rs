use std::process::ExitCode;

use anyhow::bail;
use clap::Parser;

use blueflame::env::GameVer;
use blueflame::program::{self, ProgramBuilder};

mod cli;
mod elf;
mod memory;
mod module;
mod romfs;

use cli::Cli;
use memory::Memory;
use module::ModuleData;
use romfs::Romfs;

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
    let romfs_path = cli.romfs.as_ref().map(|s| s.as_ref());
    let romfs = Romfs::find_paths(&cli.sdk_elf, romfs_path)?;
    let game_ver = if data.info.is_1_6_0 {
        GameVer::X160
    } else {
        GameVer::X150
    };

    let memory = Memory::load(cli.start, &data)?;

    let program = ProgramBuilder::new(game_ver)
        .set_program_location(cli.start, memory.get_program_size())
        .add_regions(memory.to_program_regions(&cli.regions))
        .add_data(romfs.load_actor_info_data()?)
        .build();

    println!("-- packing the program...");
    let data = program::pack(&program)?;
    println!("packed size: {} bytes", data.len());
    println!("-- verifying the pack...");
    let program2 = program::unpack(&data)?;
    if program != program2 {
        bail!("the unpacked program does not match the original program");
    }

    println!("-- writing output file: {}", cli.output);

    std::fs::write(cli.output, data)?;

    println!("done!");

    Ok(())
}

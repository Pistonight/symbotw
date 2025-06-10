use std::process::ExitCode;

use anyhow::bail;
use clap::Parser;

use blueflame::env::{DataId, GameVer};
use blueflame::program;
//use blueflame::program::{self, ProgramBuilder};

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
        eprintln!("error: {e:?}");
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

    // make the memory
    let memory = Memory::load(cli.start, &data)?;
    // build the program image
    let info = &data.info;
    let mut builder = program::builder(game_ver, cli.start, memory.get_program_size())
        .add_module("rtld", info.rtld.start)
        .add_module("main", info.main.start)
        .add_module("subsdk0", info.subsdk0.start)
        .add_module("sdk", info.sdk.start)
        .done_with_modules();
    for section in &memory.regions {
        builder = builder.add_section(section.rel_start, section.permissions);
    }
    let mut builder = builder.done_with_sections();
    builder = memory.add_program_segments(&cli.regions, builder);

    let program = builder
        .add_data(DataId::ActorInfoByml, romfs.load_actor_info_data()?)
        .done();

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

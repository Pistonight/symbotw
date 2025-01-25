use std::process::ExitCode;

use anyhow::bail;
use clap::Parser;

use blueflame_program::ProgramBuilder;
use blueflame_utils::{DlcVer, Environment, GameVer};

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

    let Some(dlc_ver) = DlcVer::from_num(cli.dlc) else {
        bail!("invalid DLC version. Must be 0-3");
    };

    // load the files
    let data = ModuleData::load(&cli.sdk_elf)?;
    let romfs_path = cli.romfs.as_ref().map(|s| s.as_ref());
    let romfs = Romfs::find_paths(&cli.sdk_elf, romfs_path)?;
    let env = Environment {
        game_ver: if data.info.is_1_6_0 {
            GameVer::X160
        } else {
            GameVer::X150
        },
        dlc_ver,
    };

    let memory = Memory::load(cli.start, &data)?;

    let program = ProgramBuilder::new(env)
        .program(
            cli.start,
            memory.get_program_size(),
            memory.to_program_regions(&cli.regions),
        )
        .add_data(romfs.load_actor_info_data()?)
        .build();

    println!("-- packing the program...");
    let data = blueflame_program::pack_blueflame(&program)?;
    println!("packed size: {} bytes", data.len());
    println!("-- verifying the pack...");
    let program2 = blueflame_program::unpack_blueflame(&data)?;
    if program != program2 {
        bail!("the unpacked program does not match the original program");
    }

    println!("-- writing the output file...");

    std::fs::write(cli.output, data)?;

    println!("done!");

    Ok(())
}

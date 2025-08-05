use cu::pre::*;

use blueflame::env::DataId;
use blueflame::program;

mod cli;
mod elf;
mod memory;
mod module;
mod romfs;

use cli::Cli;
use memory::Memory;
use module::ModuleData;
use romfs::Romfs;

#[cu::cli(flags = "common")]
fn main(cli: Cli) -> cu::Result<()> {
    if cli.start & 0xFFFFFF00000FFFFF != 0 {
        cu::bail!("invalid program start (see --help)");
    }

    // load the files
    let modules = ModuleData::load(&cli.sdk_elf).context("failed to load module data")?;
    let romfs = Romfs::find_paths(&cli.sdk_elf, cli.romfs.as_deref())
        .context("did not find all necessary files from romfs")?;

    // make the memory
    let memory = Memory::load(cli.start, &modules)?;

    cu::info!("building the program image");
    let info = &modules.info;
    let mut builder = program::builder(info.ver, cli.start, memory.get_program_size())
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

    let data = {
        let bar = cu::progress_unbounded("packing the program");
        let data = program::pack(&program).context("failed to pack program")?;
        cu::info!("packed size: {} bytes", data.len());
        cu::progress!(&bar, (), "verifying the pack");
        let program2 =
            program::unpack(&data).context("failed to unpack program for verification")?;
        if program != program2 {
            cu::bail!("the unpacked program does not match the original program");
        }
        data
    };

    cu::info!("writing output file to: {}", cli.output);

    cu::fs::write(cli.output, data).context("failed to write output file")?;

    Ok(())
}

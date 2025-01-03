# uking-relocate

Tool for simulating loading BOTW exefs into memory.

This tool is developed for building runtime images for BlueFlame,
the core that powers the IST simulator.

This tool is only useful for researching how the Switch OS loads programs.
It cannot be used to play the game or any other game. This repo does not
contain any game files in any form.

## Install
The tool is cross-platform. You need to install the Rust toolchain from https://rustup.rs/

Then install the tool from source from this repo
```
cargo install uking-relocate --git https://github.com/Pistonight/symbotw
```

## Requirements
You need to have the ExeFS dump of BOTW Switch 1.5.0 or 1.6.0, depends
on which one you want to use. The dump would contain 4 compressed modules `rtld`, 
`main`, `subsdk0` and `sdk`. The `main.npdm` file is not needed.

Then, for each of the 4 modules, you need to first decompress it,
then convert it to ELF. See [here for reference](https://github.com/open-ead/nx-decomp-tools/blob/8a19eb879e94ff19bcc5fb59c0ce3336ce3214a9/setup_common.py#L36C1-L46C79)

The ELF files needs to placed in the same directory

## Memory Layout
You need to provide an absolute offset in the physical memory space as the
starting location for loading the program. This offset is 64-bits, and must satisfy
the following:
- The upper 24 bits are 0
- The lower 20 bits are 0

In other words, it should look like `0x000000XXXXX00000` in hexadecimal.

This is where the first module (i.e. `rtld`) will be loaded.

To have complete control of the memory layout, you also need to control
the stack and heap allocation. This is the responsibility of the client
program and not this tool. For example, BlueFlame lets you specify
the stack region and the address of the heap-allocated PauseMenuDataMgr
to derive the heap region.

## Usage Cheatsheet
```
uking-relocate SDK [-o OUTPUT] --start ADDR [--dlc] [--regions ...]
```
- `SDK` is the path to the ELF version of the `sdk` module. The version string
  embedded in the SDK module is used to determine the game version.
- `OUTPUT` defaults to `program.blfm`
- `--dlc` affects singleton allocation info, see below
- `--regions` Specify resulting memory regions to keep in the output.
  See `--help` for more info

## Singleton Allocation
BOTW initializes the singletons in a predictable way. To achieve
accurate simulation, this tool provide information for some singletons
so downstream tools like BlueFlame can allocate them in the right place.

The allocations only differ in game version and if you have DLC installed.
To simulate the environment with DLC, make sure you add the `--dlc` flag.

Currently, the supported singletons are (by decomp name):
- `uking::ui::PauseMenuDataMgr`
- `ksys::gdt::Manager` a.k.a `GameDataManager`
- `ksys::act::InfoData` a.k.a `ActorInfoData`
- `ksys::act::PlayerInfo`
- `AocManager`

## Output
Currently, the only supported output is a `.blfm` image.
It needs to be decoded by the `uking-relocate-lib` crate.

Add it to dependency:
```
cargo add uking-relocate-lib --git https://github.com/Pistonight/symbotw
```
```rust
let data = std::fs::read("my_pack.blfm").unwrap();
let program = uking_relocate_lib::unpack_blueflame(&data).unwrap();
```

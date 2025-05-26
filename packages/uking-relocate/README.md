# uking-relocate

Simulates loading BOTW executable and some data into memory, producing an image for BlueFlame (the IST simulator core)

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
You need to have the Dump of BOTW Switch 1.5.0 or 1.6.0, depends on which version you want to use.
The tool automatically detects the version.

You need to dump these files: (`exefs:` and `romfs:` indicate which section you need to dump from,
followed by the path in that section)
- exefs:/main
- exefs:/subsdk0
- exefs:/sdk
- exefs:/rtld
- romfs:/Actor/ActorInfo.product.sbyml

For each of the modules in `exefs`, you need to first decompress it, then convert it to ELF. See [here for reference](https://github.com/open-ead/nx-decomp-tools/blob/8a19eb879e94ff19bcc5fb59c0ce3336ce3214a9/setup_common.py#L36C1-L46C79)

In the end, you should end up with a directory structure that looks like:
```
├─exefs
│  ├─main.elf
│  ├─subsdk0.elf
│  ├─sdk.elf
│  └─rtld.elf
└─romfs
   └─Actor
      └─ActorInfo.product.sbyml

```
The `romfs` directory can also be placed inside `exefs` and the tool will be able to find it as well.
You can also use `--romfs PATH` to specify a path manually 

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
uking-relocate SDK [-o OUTPUT] --start ADDR [--romfs PATH] [--regions ...]
```
- `SDK` is the path to the ELF version of the `sdk` module. The version string
  embedded in the SDK module is used to determine the game version.
- `--regions` Specify resulting memory regions to keep in the output.

See `--help` for more info

## Output
The output BlueFlame image can be loaded into BlueFlame or decoded
by the `blueflame-program` Rust crate for external use. 

**Currently in development, and you need to specify the `dev2` branch when running `cargo add` **

Add it to dependency:
```
cargo add blueflame-program --git https://github.com/Pistonite/botw-ist
```
```rust
let data = std::fs::read("my_pack.bfi")?;
let program = blueflame_program::unpack_blueflame(&data)?;
```

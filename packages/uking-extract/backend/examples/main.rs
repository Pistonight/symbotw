use std::path::PathBuf;
use std::process::ExitCode;

use uking_extract_backend::Options;

fn main() -> ExitCode {
    // extract the example ELF
    let options = Options {
        output: PathBuf::from("botw-decomp/build/uking-extract.yaml"),
        func: PathBuf::from("botw-decomp/data/uking_functions.csv"),
        data: PathBuf::from("botw-decomp/data/data_symbols.csv"),
        elf: PathBuf::from("botw-decomp/build/uking"),
    };

    uking_extract_common::run(|| uking_extract_backend::extract(&options))
}

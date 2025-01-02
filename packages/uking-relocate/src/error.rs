
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("the input ELF file does not match a known version of the game")]
    UnrecognizedInputELF,
    #[error("cannot find all of the game's input files: missing {0:?}")]
    MissingInputELF(Vec<&'static str>),
}

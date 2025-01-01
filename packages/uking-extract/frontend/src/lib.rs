pub struct IDAImportOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub pattern: String,
    pub type_only: bool,
    pub address: u32,
    pub name_only: bool,
    pub skip_types: bool,
    pub verbose: u32,
}

pub struct PythonBundler {

}
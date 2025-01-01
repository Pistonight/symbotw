use std::path::PathBuf;

/// Try to find the botw directory
pub fn find_botw() -> Option<PathBuf> {
    let mut current = dunce::canonicalize(".").ok()?;
    loop {
        if !current.exists() {
            return None;
        }
        let data = current.join("data");
        if data.exists() {
            let funcs = data.join("uking_functions.csv");
            let symbols = data.join("data_symbols.csv");
            if funcs.exists() && symbols.exists() {
                return Some(current);
            }
        }
        current = current.parent()?.to_path_buf();
    }
}

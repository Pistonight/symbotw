use std::path::Path;
use std::process::ExitCode;
use std::time::Instant;

mod progress;
pub use progress::ProgressPrinter;
mod find_botw;
pub use find_botw::*;

/// Main execution wrapper
pub fn run<T, E: std::fmt::Debug, F: FnOnce() -> Result<T, E>>(f: F) -> ExitCode {
    let start_time = Instant::now();
    if let Err(e) = f() {
        eprintln!("\nError: {:?}", e);
        return ExitCode::FAILURE;
    }
    let elapsed = start_time.elapsed();
    println!("Finished in {:.02} seconds", elapsed.as_secs_f32());
    ExitCode::SUCCESS
}

pub fn ensure_parent_exists(path: impl AsRef<Path>) -> std::io::Result<()> {
    if let Some(parent) = path.as_ref().parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

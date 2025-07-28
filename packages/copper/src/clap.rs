use clap::Args;

use crate::ColorLevel;

#[cfg(feature = "prompt")]
use crate::PromptLevel;

/// Common CLI flags for clap - This should be `flattened` into the cli tool using
/// it
#[derive(Debug, Clone, PartialEq, Args)]
pub struct CliFlags {
    /// Verbose. More -v makes it more verbose (opposite of --quiet)
    #[clap(short = 'v', long, action(clap::ArgAction::Count))]
    verbose: u8,
    /// Quiet. More -q makes it more quiet (opposite of --verbose)
    #[clap(short = 'q', long, action(clap::ArgAction::Count))]
    quiet: u8,
    /// Set the color mode for this program. May affect subprocesses spawned.
    #[clap(long)]
    color: Option<ColorLevel>,
    /// Automatically answer 'yes' to all yes/no prompts
    #[cfg(feature = "prompt")]
    #[clap(short = 'y', long)]
    yes: bool,
    /// Make all prompts fail with an error. (Cancels with one --interactive)
    #[cfg(feature = "prompt")]
    #[clap(long, action(clap::ArgAction::Count))]
    non_interactive: u8,
    /// Allow interactivity. Cancels with one --non-interactive
    #[cfg(feature = "prompt")]
    #[clap(long, action(clap::ArgAction::Count))]
    interactive: u8,
}

impl CliFlags {
    pub fn apply_print_options(&self) {
        let level = self.verbose.clamp(0, 2) as i8 - self.quiet.clamp(0, 2) as i8;
        let prompt =  {
            #[cfg(feature = "prompt")]
            match self.non_interactive.min(i8::MAX as u8) as i8 - self.interactive.min(i8::MAX as u8) as i8 {
                ..0 => if self.yes {
                    Some(PromptLevel::Yes)
                } else {
                    Some(PromptLevel::Interactive)
                }
                0 => if self.yes {
                    Some(PromptLevel::Yes)
                } else {
                    None
                }
                _ => Some(PromptLevel::No)
            }
            #[cfg(not(feature = "prompt"))]
            {
                None
            }
        };
        crate::init_print_options(self.color.unwrap_or_default(), level.into(), prompt);
    }
}

/// A wrapper for your main function that executes an inner function that may fail
#[inline(always)]
pub fn cli_wrapper<T: clap::Parser + AsRef<CliFlags>, F: FnOnce(T) -> crate::Result<()>>(f: F) -> std::process::ExitCode {
    let start = std::time::Instant::now();
    let args = <T as clap::Parser>::parse();
    let flags = args.as_ref();
    flags.apply_print_options();
    let result = f(args);
    let elapsed = start.elapsed().as_secs_f32();
    if let Err(e) = result {
        crate::debug!("finished in {elapsed:.2}s");
        crate::error!("fatal: {e:?}");
        if std::env::var("RUST_BACKTRACE").unwrap_or_default().is_empty() {
            crate::hint!("set RUST_BACKTRACE=1 to get backtrace for the error above.");
        }
        std::process::ExitCode::FAILURE
    } else {
        crate::info!("finished in {elapsed:.2}s");
        std::process::ExitCode::SUCCESS
    }
}

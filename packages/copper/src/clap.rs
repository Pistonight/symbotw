use clap::Args;

use crate::ColorLevel;


/// Common CLI flags for clap - This should be `flattened` into the cli tool using
/// it
#[derive(Debug, Clone, PartialEq, Args)]
pub struct CliFlags {
    /// Verbose. More -v makes it more verbose (opposite of --quiet)
    #[clap(short = 'v', long, action(clap::ArgAction::Count))]
    verbose: i8,
    /// Quiet. More -q makes it more quiet (opposite of --verbose)
    #[clap(short = 'q', long, action(clap::ArgAction::Count))]
    quiet: i8,
    /// Set the color mode for this program. May affect subprocesses spawned.
    #[clap(long)]
    color: ColorLevel,

    // TODO: yes/no
}

impl CliFlags {
    pub fn apply_print_options(&self) {
        let level = self.verbose.clamp(0, 2) - self.quiet.clamp(0, 2);
        crate::init_print_options(self.color, level.into());
    }
}

use clap::Args;


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
}

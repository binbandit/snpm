use anyhow::Result;
use clap::Args;
use snpm_core::SnpmConfig;
use snpm_core::node::current;

#[derive(Args, Debug)]
pub struct CurrentArgs {
    /// Print just the version string with no surrounding text
    #[arg(long = "quiet")]
    pub quiet: bool,
}

pub fn run(args: CurrentArgs, config: &SnpmConfig) -> Result<()> {
    match current::read_current(config)? {
        Some(version) => {
            if args.quiet {
                println!("{}", version);
            } else {
                println!("Active Node version: {}", version);
            }
        }
        None => {
            if !args.quiet {
                println!("No Node version is currently active.");
                println!();
                println!("Activate one with: snpm node use <version>");
            }
        }
    }
    Ok(())
}

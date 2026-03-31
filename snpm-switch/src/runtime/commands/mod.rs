mod cache;
mod help;
mod list;

use crate::cli::SwitchOptions;
use crate::version;

use std::process::ExitCode;

use super::binary::resolve_binary;
use cache::run_cache;
use help::print_switch_help;
use list::run_list;

pub(super) fn handle_switch_command(
    args: &[String],
    options: &SwitchOptions,
) -> anyhow::Result<ExitCode> {
    match args.first().map(|arg| arg.as_str()) {
        Some("list") => run_list(),
        Some("cache") => run_cache(&args[1..], options),
        Some("which") => {
            let path = resolve_binary(None, options)?;
            println!("{}", path.display());
            Ok(ExitCode::SUCCESS)
        }
        Some("clear") => {
            version::clear_cache()?;
            println!("Cache cleared.");
            Ok(ExitCode::SUCCESS)
        }
        Some(subcommand) => anyhow::bail!("Unknown switch subcommand: {}", subcommand),
        None => {
            print_switch_help();
            Ok(ExitCode::SUCCESS)
        }
    }
}

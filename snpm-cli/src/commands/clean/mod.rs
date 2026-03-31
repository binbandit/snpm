mod confirm;
mod options;
mod preview;

use anyhow::Result;
use clap::Args;
use snpm_core::{SnpmConfig, console, operations};

use confirm::prompt_confirmation;
use options::build_options;
use preview::{pluralize, print_preview};

#[derive(Args, Debug)]
pub struct CleanArgs {
    /// Skip confirmation prompt
    #[arg(short = 'y', long = "yes")]
    pub yes: bool,

    /// Show what would be deleted without deleting
    #[arg(long = "dry-run")]
    pub dry_run: bool,

    /// Only clean cached packages
    #[arg(long = "packages")]
    pub packages: bool,

    /// Only clean registry metadata cache
    #[arg(long = "metadata")]
    pub metadata: bool,

    /// Also clean global packages and binaries
    #[arg(long = "global")]
    pub global: bool,

    /// Clean everything (packages, metadata, and global)
    #[arg(long = "all")]
    pub all: bool,
}

pub async fn run(args: CleanArgs, config: &SnpmConfig) -> Result<()> {
    console::header("clean", env!("CARGO_PKG_VERSION"));

    let options = build_options(&args);
    let summary = operations::clean_analyze(config, &options)?;

    if summary.is_empty() {
        println!("Nothing to clean.");
        return Ok(());
    }

    print_preview(&summary, &options);

    if args.dry_run {
        println!();
        console::info("Dry run complete. No files were deleted.");
        return Ok(());
    }

    if !args.yes && !prompt_confirmation()? {
        console::info("Aborted.");
        return Ok(());
    }

    println!();
    operations::clean_execute(config, &options)?;

    println!();
    console::info(&format!(
        "Cleaned {} ({} freed)",
        pluralize(summary.total_count(), "item", "items"),
        operations::format_bytes(summary.total_size())
    ));

    Ok(())
}

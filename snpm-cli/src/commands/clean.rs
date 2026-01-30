use anyhow::Result;
use clap::Args;
use snpm_core::{SnpmConfig, console, operations};
use std::env;
use std::io::{self, BufRead, Write};

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

fn build_options(args: &CleanArgs) -> operations::CleanOptions {
    if args.all {
        return operations::CleanOptions::all();
    }

    let has_explicit_selection = args.packages || args.metadata || args.global;

    if has_explicit_selection {
        operations::CleanOptions {
            packages: args.packages,
            metadata: args.metadata,
            global: args.global,
        }
    } else {
        operations::CleanOptions::default()
    }
}

fn print_preview(summary: &operations::CleanSummary, options: &operations::CleanOptions) {
    println!("The following will be removed:");
    println!();

    if options.packages && summary.packages_count > 0 {
        println!(
            "  Cached packages:    {:>5} {}  ({})",
            summary.packages_count,
            pluralize(summary.packages_count, "package", "packages"),
            operations::format_bytes(summary.packages_size)
        );
    }

    if options.metadata && summary.metadata_count > 0 {
        println!(
            "  Metadata cache:     {:>5} {}  ({})",
            summary.metadata_count,
            pluralize(summary.metadata_count, "entry", "entries"),
            operations::format_bytes(summary.metadata_size)
        );
    }

    if options.global && summary.global_count > 0 {
        println!(
            "  Global installs:    {:>5} {}   ({})",
            summary.global_count,
            pluralize(summary.global_count, "item", "items"),
            operations::format_bytes(summary.global_size)
        );
    }

    println!();
    println!(
        "  Total:              {:>5} {}  ({})",
        summary.total_count(),
        pluralize(summary.total_count(), "item", "items"),
        operations::format_bytes(summary.total_size())
    );
}

fn prompt_confirmation() -> Result<bool> {
    println!();
    print!("Continue? [y/N] ");
    io::stdout().flush()?;

    let stdin = io::stdin();
    let mut input = String::new();
    stdin.lock().read_line(&mut input)?;

    let response = input.trim().to_lowercase();
    Ok(response == "y" || response == "yes")
}

fn pluralize<'a>(count: usize, singular: &'a str, plural: &'a str) -> &'a str {
    if count == 1 { singular } else { plural }
}

use anyhow::Result;
use clap::Subcommand;
use snpm_core::{SnpmConfig, console, operations};

#[derive(clap::Args, Debug)]
pub struct StoreArgs {
    #[command(subcommand)]
    pub command: StoreCommand,
}

#[derive(Subcommand, Debug)]
pub enum StoreCommand {
    /// Show store disk usage
    Status,
    /// Remove incomplete or orphaned packages from the store
    Prune(PruneArgs),
    /// Print the store path
    Path,
}

#[derive(clap::Args, Debug)]
pub struct PruneArgs {
    /// Show what would be removed without removing
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

pub async fn run(args: StoreArgs, config: &SnpmConfig) -> Result<()> {
    match args.command {
        StoreCommand::Status => {
            let status = operations::store_status(config)?;
            println!("Store path: {}", status.store_path);
            println!();
            println!(
                "Packages:  {} ({})",
                status.packages_count,
                operations::format_bytes(status.packages_size)
            );
            println!(
                "Metadata:  {} ({})",
                status.metadata_count,
                operations::format_bytes(status.metadata_size)
            );
            println!(
                "Total:     {}",
                operations::format_bytes(status.packages_size + status.metadata_size)
            );
        }
        StoreCommand::Prune(prune_args) => {
            let pruned = operations::store_prune(config, prune_args.dry_run)?;
            if pruned == 0 {
                println!("Store is clean, nothing to prune.");
            } else if prune_args.dry_run {
                println!();
                console::info(&format!("Would remove {} incomplete packages.", pruned));
            } else {
                console::info(&format!("Pruned {} incomplete packages.", pruned));
            }
        }
        StoreCommand::Path => {
            println!("{}", operations::store_path(config));
        }
    }
    Ok(())
}

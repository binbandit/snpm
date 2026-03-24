use anyhow::{Context, Result};
use clap::Args;
use snpm_core::{Project, SnpmConfig, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct PublishArgs {
    /// Distribution tag for this release
    #[arg(long, default_value = "latest")]
    pub tag: String,

    /// Package access level (public or restricted)
    #[arg(long)]
    pub access: Option<String>,

    /// One-time password for 2FA
    #[arg(long)]
    pub otp: Option<String>,

    /// Show what would be published without publishing
    #[arg(long = "dry-run")]
    pub dry_run: bool,
}

pub async fn run(args: PublishArgs, config: &SnpmConfig) -> Result<()> {
    let cwd = env::current_dir().context("failed to determine current directory")?;
    let project = Project::discover(&cwd)?;

    console::step("Packing");
    let pack_result = operations::pack(&project, &cwd)?;

    console::info(&format!(
        "Packed {} files ({})",
        pack_result.file_count,
        operations::format_bytes(pack_result.size)
    ));

    let options = operations::PublishOptions {
        tag: args.tag,
        access: args.access,
        otp: args.otp,
        dry_run: args.dry_run,
    };

    operations::publish(config, &project, &pack_result.tarball_path, &options).await?;

    // Clean up tarball after successful publish
    if !args.dry_run {
        std::fs::remove_file(&pack_result.tarball_path).ok();
    }

    Ok(())
}

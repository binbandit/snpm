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

    /// Allow a specific blocking risk code for this publish only
    #[arg(long = "allow-risk", value_name = "CODE")]
    pub allow_risks: Vec<String>,
}

pub async fn run(args: PublishArgs, config: &SnpmConfig) -> Result<()> {
    let cwd = env::current_dir().context("failed to determine current directory")?;
    let project = Project::discover(&cwd)?;

    console::step("Packing");
    let pack_result = operations::pack(&project, &cwd)?;
    print_findings(&pack_result.inspection.findings);

    console::info(&format!(
        "Packed {} files ({} packed, {} unpacked)",
        pack_result.file_count(),
        operations::format_bytes(pack_result.packed_size),
        operations::format_bytes(pack_result.inspection.unpacked_size)
    ));

    let options = operations::PublishOptions {
        tag: args.tag,
        access: args.access,
        otp: args.otp,
        dry_run: args.dry_run,
        allow_risks: args.allow_risks,
    };

    operations::publish(
        config,
        &project,
        &pack_result.inspection,
        &pack_result.tarball_path,
        &options,
    )
    .await?;

    // Clean up tarball after successful publish
    if !args.dry_run {
        std::fs::remove_file(&pack_result.tarball_path).ok();
    }

    Ok(())
}

fn print_findings(findings: &[operations::PackFinding]) {
    for finding in findings {
        let message = match finding.path.as_deref() {
            Some(path) => format!("{}: {} ({})", finding.code, finding.message, path),
            None => format!("{}: {}", finding.code, finding.message),
        };

        if finding.is_blocking() {
            console::error(&message);
        } else {
            console::warn(&message);
        }
    }
}

use anyhow::{Context, Result};
use clap::Args;
use snpm_core::{Project, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct PackArgs {
    /// Show what would be packed without writing a tarball
    #[arg(long = "dry-run")]
    pub dry_run: bool,

    /// List packed files
    #[arg(long)]
    pub list: bool,

    /// Output package contents in JSON format
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: PackArgs) -> Result<()> {
    let cwd = env::current_dir().context("failed to determine current directory")?;
    let project = Project::discover(&cwd)?;

    if args.dry_run {
        let inspection = operations::inspect_pack(&project)?;
        if args.json {
            println!("{}", serde_json::to_string_pretty(&inspection)?);
            return Ok(());
        }

        print_findings(&inspection.findings);
        if args.list || args.dry_run {
            print_files(&inspection.files);
        }
        console::info(&format!(
            "Would pack {} files ({})",
            inspection.file_count(),
            operations::format_bytes(inspection.unpacked_size)
        ));
        return Ok(());
    }

    let result = operations::pack(&project, &cwd)?;

    if args.json {
        println!("{}", serde_json::to_string_pretty(&result)?);
        return Ok(());
    }

    print_findings(&result.inspection.findings);
    if args.list {
        print_files(&result.inspection.files);
    }

    console::info(&format!(
        "Packed {} files into {} ({} packed, {} unpacked)",
        result.file_count(),
        result.tarball_path.display(),
        operations::format_bytes(result.packed_size),
        operations::format_bytes(result.inspection.unpacked_size)
    ));

    Ok(())
}

fn print_files(files: &[operations::PackFile]) {
    for file in files {
        println!(
            "packed {} {} [{}]",
            operations::format_bytes(file.size),
            file.path,
            format_reason(&file.reason)
        );
    }
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

fn format_reason(reason: &operations::PackFileReason) -> &'static str {
    match reason {
        operations::PackFileReason::Manifest => "manifest",
        operations::PackFileReason::ManifestFiles => "files",
        operations::PackFileReason::DefaultScan => "default",
        operations::PackFileReason::Mandatory => "mandatory",
        operations::PackFileReason::MainEntry => "main",
        operations::PackFileReason::BinEntry => "bin",
    }
}

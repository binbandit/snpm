use anyhow::{Context, Result};
use clap::Args;
use snpm_core::{Project, operations};
use std::env;

#[derive(Args, Debug)]
pub struct LicensesArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: LicensesArgs) -> Result<()> {
    let cwd = env::current_dir().context("failed to determine current directory")?;
    let project = Project::discover(&cwd)?;
    let node_modules = project.root.join("node_modules");

    let entries = operations::collect_licenses(&node_modules)?;

    if entries.is_empty() {
        println!("No packages found.");
        return Ok(());
    }

    if args.json {
        println!("{}", serde_json::to_string_pretty(&entries)?);
        return Ok(());
    }

    // Table output
    let max_name = entries
        .iter()
        .map(|e| e.name.len())
        .max()
        .unwrap_or(10)
        .max(7);
    let max_ver = entries
        .iter()
        .map(|e| e.version.len())
        .max()
        .unwrap_or(7)
        .max(7);

    println!(
        "{:<width_name$}  {:<width_ver$}  License",
        "Package",
        "Version",
        width_name = max_name,
        width_ver = max_ver,
    );
    println!(
        "{:<width_name$}  {:<width_ver$}  ───────",
        "───────",
        "───────",
        width_name = max_name,
        width_ver = max_ver,
    );

    for entry in &entries {
        println!(
            "{:<width_name$}  {:<width_ver$}  {}",
            entry.name,
            entry.version,
            entry.license,
            width_name = max_name,
            width_ver = max_ver,
        );
    }

    println!();
    println!("{} packages found.", entries.len());

    Ok(())
}

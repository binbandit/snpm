mod config;
mod fix;
mod output;
mod target;

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};
use snpm_core::{SnpmConfig, console};
use std::env;

use config::build_audit_options;
use fix::run_fix;
use output::{print_json, print_sarif, print_table};
use target::run_audit;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable table output (default)
    Table,
    /// JSON output for programmatic use
    Json,
    /// SARIF format for GitHub/GitLab security integration
    Sarif,
}

#[derive(Args, Debug)]
pub struct AuditArgs {
    /// Only report vulnerabilities at or above this severity
    #[arg(long, value_name = "LEVEL")]
    pub audit_level: Option<String>,

    /// Only audit production dependencies (no devDependencies)
    #[arg(long, short = 'P', conflicts_with = "dev")]
    pub prod: bool,

    /// Only audit devDependencies
    #[arg(long, short = 'D', conflicts_with = "prod")]
    pub dev: bool,

    /// Output format (table, json, sarif)
    #[arg(long, value_enum, default_value = "table")]
    pub format: OutputFormat,

    /// Attempt to fix vulnerabilities by updating packages
    #[arg(long)]
    pub fix: bool,

    /// Ignore vulnerabilities by CVE ID (can be specified multiple times)
    #[arg(long = "ignore-cve", value_name = "CVE")]
    pub ignore_cves: Vec<String>,

    /// Ignore vulnerabilities by GHSA ID (can be specified multiple times)
    #[arg(long = "ignore-ghsa", value_name = "GHSA")]
    pub ignore_ghsas: Vec<String>,

    /// Ignore vulnerabilities with no available fix
    #[arg(long)]
    pub ignore_unfixable: bool,

    /// Continue with exit code 0 even if registry returns an error
    #[arg(long)]
    pub ignore_registry_errors: bool,

    /// Audit specific packages only
    #[arg(value_name = "PACKAGE")]
    pub packages: Vec<String>,
}

pub async fn run(args: AuditArgs, config: &SnpmConfig) -> Result<()> {
    console::header("audit", env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir().context("failed to determine current directory")?;
    let (options, audit_level) = build_audit_options(&args)?;

    if args.fix {
        return run_fix(config, &cwd, &options).await;
    }

    let results = match run_audit(config, &cwd, &options).await {
        Ok(results) => results,
        Err(e) if args.ignore_registry_errors => {
            console::warn(&format!("Audit failed: {}", e));
            return Ok(());
        }
        Err(e) => return Err(e.into()),
    };

    let has_vulnerabilities = match args.format {
        OutputFormat::Table => print_table(&results, audit_level),
        OutputFormat::Json => print_json(&results)?,
        OutputFormat::Sarif => print_sarif(&results)?,
    };

    if has_vulnerabilities {
        anyhow::bail!("vulnerabilities found");
    }

    Ok(())
}

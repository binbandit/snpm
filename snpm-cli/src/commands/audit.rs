use anyhow::Result;
use clap::{Args, ValueEnum};
use snpm_core::{Project, SnpmConfig, Workspace, console, operations};
use std::collections::HashSet;
use std::env;
use std::process;

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
    #[arg(long, short = 'P')]
    pub prod: bool,

    /// Only audit devDependencies
    #[arg(long, short = 'D')]
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

    let cwd = env::current_dir()?;

    let audit_level = args
        .audit_level
        .as_deref()
        .map(|s| s.parse::<operations::Severity>())
        .transpose()
        .map_err(|e| anyhow::anyhow!("{}", e))?;

    let options = operations::AuditOptions {
        audit_level,
        production_only: args.prod,
        dev_only: args.dev,
        packages: args.packages.clone(),
        ignore_cves: args.ignore_cves.iter().cloned().collect::<HashSet<_>>(),
        ignore_ghsas: args.ignore_ghsas.iter().cloned().collect::<HashSet<_>>(),
        ignore_unfixable: args.ignore_unfixable,
    };

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
        process::exit(1);
    }

    Ok(())
}

/// Run audit for the project or workspace at the given path.
async fn run_audit(
    config: &SnpmConfig,
    cwd: &std::path::Path,
    options: &operations::AuditOptions,
) -> std::result::Result<Vec<operations::AuditResult>, snpm_core::SnpmError> {
    if let Some(workspace) = Workspace::discover(cwd)?
        && workspace.root == *cwd
    {
        operations::audit_workspace(config, &workspace, options).await
    } else {
        let project = Project::discover(cwd)?;
        let result = operations::audit(config, &project, options).await?;
        Ok(vec![result])
    }
}

async fn run_fix(
    config: &SnpmConfig,
    cwd: &std::path::Path,
    options: &operations::AuditOptions,
) -> Result<()> {
    let project = Project::discover(cwd)?;
    let result = operations::fix(config, &project, options).await?;

    if result.fixed.is_empty() && result.unfixable.is_empty() {
        console::info("No vulnerabilities found.");
        return Ok(());
    }

    if !result.fixed.is_empty() {
        println!();
        println!(
            "{}",
            paint(
                "32;1",
                &format!("Fixed {} vulnerabilities:", result.fixed.len())
            )
        );
        println!();

        for fix in &result.fixed {
            println!(
                "  {} {} {} -> {}",
                severity_badge(fix.severity),
                fix.package,
                paint("2", &fix.from_version),
                paint("32", &fix.to_version),
            );
        }
    }

    if !result.unfixable.is_empty() {
        println!();
        println!(
            "{}",
            paint(
                "33;1",
                &format!(
                    "{} vulnerabilities cannot be fixed automatically:",
                    result.unfixable.len()
                ),
            )
        );
        println!();

        for entry in &result.unfixable {
            println!(
                "  {} {}@{} - {}",
                severity_badge(entry.severity),
                entry.package,
                entry.version,
                paint("2", &entry.reason),
            );
        }
    }

    println!();
    if !result.fixed.is_empty() {
        console::info("Run `snpm install` to apply fixes.");
    }

    Ok(())
}

// ============================================================================
// Output: table
// ============================================================================

fn print_table(
    results: &[operations::AuditResult],
    threshold: Option<operations::Severity>,
) -> bool {
    let mut total_counts = operations::VulnerabilityCounts::default();
    let mut total_packages = 0;
    let mut any_vulnerabilities = false;

    for result in results {
        total_packages += result.total_packages;
        total_counts.merge(&result.counts);

        if let Some(member) = &result.workspace_member {
            println!();
            println!("{}", paint("1", member));
        }

        for advisory in &result.advisories {
            any_vulnerabilities = true;
            print_advisory(advisory);
        }
    }

    println!();
    print_summary(&total_counts, total_packages);

    // Respect audit_level for exit code (pnpm gets this wrong)
    if let Some(t) = threshold {
        total_counts.above_threshold(t) > 0
    } else {
        any_vulnerabilities
    }
}

fn print_advisory(advisory: &operations::AuditAdvisory) {
    let width = terminal_width();

    println!();

    // Severity badge + title
    println!(
        "{} {}",
        severity_badge(advisory.severity),
        paint("1", &advisory.title),
    );

    // Package
    println!("  {} {}", paint("2", "Package:"), advisory.module_name);

    // Vulnerable versions
    println!(
        "  {} {}",
        paint("2", "Vulnerable:"),
        paint("31", &advisory.vulnerable_versions),
    );

    // Patched versions
    let patched = if advisory.patched_versions.is_empty() || advisory.patched_versions == "<0.0.0" {
        paint("33", "No fix available")
    } else {
        paint("32", &advisory.patched_versions)
    };
    println!("  {} {}", paint("2", "Patched:"), patched);

    // CVEs
    if !advisory.cves.is_empty() {
        println!("  {} {}", paint("2", "CVE:"), advisory.cves.join(", "));
    }

    // Dependency paths (show all - pnpm only shows 3)
    if !advisory.findings.is_empty() {
        println!("  {}", paint("2", "Paths:"));
        for finding in &advisory.findings {
            for path in &finding.paths {
                let formatted = path.replace('>', " > ");
                print_wrapped(&formatted, width, 4);
            }
        }
    }

    // More info link
    if let Some(url) = &advisory.url {
        println!("  {} {}", paint("2", "More info:"), paint("36", url));
    } else if let Some(ghsa) = &advisory.github_advisory_id {
        let url = format!("https://github.com/advisories/{}", ghsa);
        println!("  {} {}", paint("2", "More info:"), paint("36", &url));
    }
}

fn print_summary(counts: &operations::VulnerabilityCounts, total_packages: usize) {
    let total = counts.total();

    if total == 0 {
        println!(
            "{}",
            paint(
                "32;1",
                &format!("No vulnerabilities found in {} packages!", total_packages),
            ),
        );
        return;
    }

    let noun = if total == 1 {
        "vulnerability"
    } else {
        "vulnerabilities"
    };
    println!(
        "{} {} found in {} packages",
        paint("31;1", &total.to_string()),
        noun,
        total_packages,
    );

    let mut parts = Vec::new();
    if counts.critical > 0 {
        parts.push(format!(
            "{} {}",
            paint("31;1", &counts.critical.to_string()),
            paint("31", "critical"),
        ));
    }
    if counts.high > 0 {
        parts.push(format!(
            "{} {}",
            paint("91", &counts.high.to_string()),
            paint("91", "high"),
        ));
    }
    if counts.moderate > 0 {
        parts.push(format!(
            "{} {}",
            paint("33", &counts.moderate.to_string()),
            paint("33", "moderate"),
        ));
    }
    if counts.low > 0 {
        parts.push(format!(
            "{} {}",
            paint("32", &counts.low.to_string()),
            paint("32", "low"),
        ));
    }
    if counts.info > 0 {
        parts.push(format!(
            "{} {}",
            paint("36", &counts.info.to_string()),
            paint("36", "info"),
        ));
    }

    if !parts.is_empty() {
        println!("Severity: {}", parts.join(" | "));
    }

    println!();
    println!(
        "{}",
        paint(
            "2",
            "Run `snpm audit --fix` to fix vulnerabilities with available patches.",
        ),
    );
}

// ============================================================================
// Output: JSON
// ============================================================================

fn print_json(results: &[operations::AuditResult]) -> Result<bool> {
    let mut has_vulnerabilities = false;

    if results.len() == 1 {
        let result = &results[0];
        has_vulnerabilities = !result.advisories.is_empty();
        println!("{}", serde_json::to_string_pretty(&result.to_json_value())?);
    } else {
        let outputs: Vec<_> = results
            .iter()
            .map(|r| {
                if !r.advisories.is_empty() {
                    has_vulnerabilities = true;
                }
                serde_json::json!({
                    "project": r.project_name,
                    "workspaceMember": r.workspace_member,
                    "audit": r.to_json_value(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&outputs)?);
    }

    Ok(has_vulnerabilities)
}

// ============================================================================
// Output: SARIF
// ============================================================================

fn print_sarif(results: &[operations::AuditResult]) -> Result<bool> {
    let mut has_vulnerabilities = false;
    let mut all_rules = Vec::new();
    let mut all_results = Vec::new();

    for result in results {
        if !result.advisories.is_empty() {
            has_vulnerabilities = true;
        }

        let sarif = result.to_sarif();
        if let Some(run) = sarif.runs.first() {
            all_rules.extend(run.tool.driver.rules.clone());
            all_results.extend(run.results.clone());
        }
    }

    let combined = operations::audit::SarifReport {
        schema: "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json".to_string(),
        version: "2.1.0".to_string(),
        runs: vec![operations::audit::SarifRun {
            tool: operations::audit::SarifTool {
                driver: operations::audit::SarifDriver {
                    name: "snpm-audit".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    information_uri: "https://github.com/nicolo-ribaudo/snpm".to_string(),
                    rules: all_rules,
                },
            },
            results: all_results,
        }],
    };

    println!("{}", serde_json::to_string_pretty(&combined)?);
    Ok(has_vulnerabilities)
}

// ============================================================================
// Helpers
// ============================================================================

fn use_color() -> bool {
    env::var_os("NO_COLOR").is_none()
}

fn paint(code: &str, text: &str) -> String {
    if use_color() {
        format!("\x1b[{}m{}\x1b[0m", code, text)
    } else {
        text.to_string()
    }
}

fn severity_badge(severity: operations::Severity) -> String {
    let (color, label) = match severity {
        operations::Severity::Critical => ("41;97", " CRITICAL "),
        operations::Severity::High => ("101;30", "   HIGH   "),
        operations::Severity::Moderate => ("43;30", " MODERATE "),
        operations::Severity::Low => ("42;30", "   LOW    "),
        operations::Severity::Info => ("46;30", "   INFO   "),
    };

    if use_color() {
        format!("\x1b[{}m{}\x1b[0m", color, label)
    } else {
        format!("[{}]", severity.as_str().to_uppercase())
    }
}

fn terminal_width() -> usize {
    env::var("COLUMNS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(80)
}

/// Print a dim string, wrapping at char boundaries if it exceeds width.
fn print_wrapped(text: &str, width: usize, indent: usize) {
    let prefix = " ".repeat(indent);
    let usable = width.saturating_sub(indent);

    if usable == 0 || text.len() <= usable {
        println!("{}{}", prefix, paint("2", text));
        return;
    }

    // Split at char boundaries to avoid panics on multi-byte UTF-8
    let mut remaining = text;
    while !remaining.is_empty() {
        let end = char_boundary(remaining, usable);
        let (line, rest) = remaining.split_at(end);
        println!("{}{}", prefix, paint("2", line));
        remaining = rest;
    }
}

/// Find the largest char boundary <= max_bytes within a string.
fn char_boundary(s: &str, max_bytes: usize) -> usize {
    if max_bytes >= s.len() {
        return s.len();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    end
}

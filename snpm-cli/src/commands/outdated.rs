use super::workspace::{self as workspace_selector, WorkspaceSelection};
use anyhow::{Context, Result};
use clap::Args;
use snpm_core::{Project, SnpmConfig, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct OutdatedArgs {
    /// Skip devDependencies
    #[arg(long)]
    pub production: bool,
    /// Run in all workspace projects
    #[arg(short = 'r', long)]
    pub recursive: bool,
    /// Filter workspace projects (name, glob, path, or dependency graph selector)
    #[arg(long)]
    pub filter: Vec<String>,
    /// Production-only filter (same selector syntax as --filter)
    #[arg(long)]
    pub filter_prod: Vec<String>,
    /// Emit results as JSON
    #[arg(long)]
    pub json: bool,
}

/// One project's outdated report, paired with a display label.
struct ProjectReport {
    label: String,
    entries: Vec<operations::OutdatedEntry>,
}

pub async fn run(args: OutdatedArgs, config: &SnpmConfig) -> Result<()> {
    if !args.json {
        console::header("outdated", env!("CARGO_PKG_VERSION"));
    }

    let cwd = env::current_dir().context("failed to determine current directory")?;
    let reports = collect_reports(&args, config, &cwd).await?;

    if args.json {
        print_json(&reports);
        return Ok(());
    }

    let mut any = false;
    for report in &reports {
        if report.entries.is_empty() {
            continue;
        }
        if any {
            println!();
        }
        any = true;
        if !report.label.is_empty() {
            println!("{}", report.label);
        }
        print_outdated(&report.entries);
    }

    if !any {
        console::info("All dependencies are up to date.");
    }

    Ok(())
}

/// Resolve the target projects and gather each one's outdated entries.
async fn collect_reports(
    args: &OutdatedArgs,
    config: &SnpmConfig,
    cwd: &std::path::Path,
) -> Result<Vec<ProjectReport>> {
    let include_dev = !args.production;

    if let Some(WorkspaceSelection {
        projects,
        filter_label: _,
    }) = workspace_selector::select_workspace_projects(
        cwd,
        "outdated",
        args.recursive,
        &args.filter,
        &args.filter_prod,
    )? {
        let mut reports = Vec::new();
        for project in projects.into_iter() {
            let entries = operations::outdated(config, &project, include_dev, false).await?;
            reports.push(ProjectReport {
                label: workspace_selector::project_label(&project),
                entries,
            });
        }
        return Ok(reports);
    }

    if let Some(workspace) = snpm_core::Workspace::discover(cwd)?
        && workspace.root == *cwd
    {
        let mut reports = Vec::new();
        for project in workspace.projects.iter() {
            let entries = operations::outdated(config, project, include_dev, false).await?;
            reports.push(ProjectReport {
                label: workspace_selector::project_label(project),
                entries,
            });
        }
        return Ok(reports);
    }

    let project = Project::discover(cwd)?;
    let entries = operations::outdated(config, &project, include_dev, false).await?;
    Ok(vec![ProjectReport {
        label: workspace_selector::project_label(&project),
        entries,
    }])
}

fn print_json(reports: &[ProjectReport]) {
    let mut rows = Vec::new();
    for report in reports {
        for entry in &report.entries {
            rows.push(serde_json::json!({
                "name": entry.name,
                "current": entry.current,
                "wanted": entry.wanted,
                "latest": entry.latest,
                "project": report.label,
            }));
        }
    }

    match serde_json::to_string_pretty(&serde_json::Value::Array(rows)) {
        Ok(text) => println!("{text}"),
        Err(_) => println!("[]"),
    }
}

fn print_outdated(entries: &[operations::OutdatedEntry]) {
    if entries.is_empty() {
        return;
    }

    let mut name_width = 4;
    for entry in entries {
        if entry.name.len() > name_width {
            name_width = entry.name.len();
        }
    }

    println!(
        "{:<name_width$}  {:<10}  {:<10}  {:<10}",
        "name",
        "current",
        "wanted",
        "latest",
        name_width = name_width
    );

    for entry in entries {
        let current = entry.current.as_deref().unwrap_or("-");
        let latest = entry.latest.as_deref().unwrap_or("-");
        println!(
            "{:<name_width$}  {:<10}  {:<10}  {:<10}",
            entry.name,
            current,
            entry.wanted,
            latest,
            name_width = name_width
        );
    }
}

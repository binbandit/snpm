mod render;
mod target;

use super::workspace::{self as workspace_selector, WorkspaceSelection};
use anyhow::{Context, Result};
use clap::Args;
use serde::Serialize;
use snpm_core::{Project, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct WhyArgs {
    /// Package name or pattern (supports `*`)
    pub package: String,

    /// Maximum reverse dependency depth
    #[arg(long)]
    pub depth: Option<usize>,

    /// Output JSON
    #[arg(long)]
    pub json: bool,

    /// Run in all workspace projects
    #[arg(short = 'r', long = "recursive")]
    pub recursive: bool,
    /// Filter workspace projects (name, glob, path, or dependency graph selector)
    #[arg(long = "filter")]
    pub filter: Vec<String>,
    /// Production-only filter (same selector syntax as --filter)
    #[arg(long = "filter-prod")]
    pub filter_prod: Vec<String>,
}

pub async fn run(args: WhyArgs) -> Result<()> {
    if !args.json {
        console::header("why", env!("CARGO_PKG_VERSION"));
    }

    let cwd = env::current_dir().context("failed to determine current directory")?;

    if let Some(WorkspaceSelection {
        projects,
        filter_label: _,
    }) = workspace_selector::select_workspace_projects(
        &cwd,
        "why",
        args.recursive,
        &args.filter,
        &args.filter_prod,
    )? {
        run_filtered(&args, &projects)?;
        return Ok(());
    }

    if target::try_run_workspace(&cwd, &args)? {
        return Ok(());
    }

    let result = target::run_project(&cwd, &args)?;
    if result.matches.is_empty() {
        render::print_no_results(&args.package);
        return Ok(());
    }

    render::print_result(&result, args.json)
}

#[derive(Serialize)]
struct WorkspaceWhyEntry {
    project: String,
    result: operations::WhyResult,
}

fn run_filtered(args: &WhyArgs, projects: &[Project]) -> Result<()> {
    if args.json {
        let mut workspace_results = Vec::new();
        for project in projects {
            let result = operations::why(
                project,
                std::slice::from_ref(&args.package),
                operations::WhyOptions { depth: args.depth },
            )?;
            if result.matches.is_empty() {
                continue;
            }
            workspace_results.push(WorkspaceWhyEntry {
                project: workspace_selector::project_label(project),
                result,
            });
        }

        if workspace_results.is_empty() {
            render::print_no_results(&args.package);
            return Ok(());
        }

        println!("{}", serde_json::to_string_pretty(&workspace_results)?);
        return Ok(());
    }

    let mut any = false;
    for project in projects {
        let result = operations::why(
            project,
            std::slice::from_ref(&args.package),
            operations::WhyOptions { depth: args.depth },
        )?;
        if result.matches.is_empty() {
            continue;
        }
        if any {
            println!();
        }
        any = true;
        println!("{}", workspace_selector::project_label(project));
        render::print_result(&result, false)?;
    }

    if !any {
        render::print_no_results(&args.package);
    }

    Ok(())
}

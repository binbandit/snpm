use anyhow::Result;
use clap::Args;
use serde::Serialize;
use snpm_core::{Project, Workspace, console, operations};
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
}

#[derive(Serialize)]
struct WorkspaceWhyEntry {
    project: String,
    result: operations::WhyResult,
}

pub async fn run(args: WhyArgs) -> Result<()> {
    if !args.json {
        console::header("why", env!("CARGO_PKG_VERSION"));
    }

    let cwd = env::current_dir()?;

    if let Some(workspace) = Workspace::discover(&cwd)?
        && workspace.root == cwd
    {
        let mut workspace_results = Vec::new();
        let mut any = false;

        for project in &workspace.projects {
            let result = operations::why(
                project,
                std::slice::from_ref(&args.package),
                operations::WhyOptions { depth: args.depth },
            )?;

            if result.matches.is_empty() {
                continue;
            }

            let project_name = project
                .manifest
                .name
                .clone()
                .unwrap_or_else(|| project.root.display().to_string());

            if args.json {
                workspace_results.push(WorkspaceWhyEntry {
                    project: project_name,
                    result,
                });
                continue;
            }

            if any {
                println!();
            }
            any = true;

            println!("{}", project_name);
            print_result(&result, false)?;
        }

        if args.json {
            println!("{}", serde_json::to_string_pretty(&workspace_results)?);
            return Ok(());
        }

        if !any {
            console::info(&format!(
                "No dependency paths found for '{}'.",
                args.package
            ));
        }

        return Ok(());
    }

    let project = Project::discover(&cwd)?;
    let result = operations::why(
        &project,
        std::slice::from_ref(&args.package),
        operations::WhyOptions { depth: args.depth },
    )?;

    if result.matches.is_empty() {
        console::info(&format!(
            "No dependency paths found for '{}'.",
            args.package
        ));
        return Ok(());
    }

    print_result(&result, args.json)
}

fn print_result(result: &operations::WhyResult, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(result)?);
        return Ok(());
    }

    for (idx, matched) in result.matches.iter().enumerate() {
        if idx > 0 {
            println!();
        }

        println!("{}@{}", matched.name, matched.version);

        if matched.paths.is_empty() {
            println!("  (no dependents found)");
            continue;
        }

        for (path_idx, path) in matched.paths.iter().enumerate() {
            if path_idx > 0 {
                println!();
            }

            if path.hops.is_empty() {
                println!("  (no parent packages found)");
                continue;
            }

            for (hop_idx, hop) in path.hops.iter().enumerate() {
                let indent = "  ".repeat(hop_idx + 1);

                match hop {
                    operations::WhyHop::Package { name, version, via } => {
                        println!("{}<- {}@{} (via {})", indent, name, version, via);
                    }
                    operations::WhyHop::Root { name, requested } => {
                        println!("{}<- root:{} (requested {})", indent, name, requested);
                    }
                }
            }

            if path.truncated {
                let indent = "  ".repeat(path.hops.len() + 1);
                println!("{}<- ... (max depth reached)", indent);
            }
        }
    }

    Ok(())
}

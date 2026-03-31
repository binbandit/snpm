use anyhow::Result;
use serde::Serialize;
use snpm_core::{Project, Workspace, operations};

use std::path::Path;

use super::WhyArgs;
use super::render::{print_no_results, print_result};

#[derive(Serialize)]
struct WorkspaceWhyEntry {
    project: String,
    result: operations::WhyResult,
}

pub(super) fn try_run_workspace(cwd: &Path, args: &WhyArgs) -> Result<bool> {
    if let Some(workspace) = Workspace::discover(cwd)?
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
        } else if !any {
            print_no_results(&args.package);
        }

        return Ok(true);
    }

    Ok(false)
}

pub(super) fn run_project(cwd: &Path, args: &WhyArgs) -> Result<operations::WhyResult> {
    let project = Project::discover(cwd)?;
    Ok(operations::why(
        &project,
        std::slice::from_ref(&args.package),
        operations::WhyOptions { depth: args.depth },
    )?)
}

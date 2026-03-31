use super::filters::{matches_filters, project_label};
use super::process::{build_path, join_args, make_command, make_direct_command};
use crate::{Project, Result, SnpmError, Workspace, console};

pub struct ExecOptions<'a> {
    pub command: &'a str,
    pub args: &'a [String],
    pub shell_mode: bool,
}

pub fn exec_command(project: &Project, options: &ExecOptions) -> Result<()> {
    let bin_dir = project.root.join("node_modules").join(".bin");
    let path_value = build_path(bin_dir, options.command)?;

    let package_name = project.manifest.name.as_deref().unwrap_or_default();
    let full_command = if options.args.is_empty() {
        options.command.to_string()
    } else {
        format!("{} {}", options.command, join_args(options.args))
    };

    console::info(&full_command);

    let mut process = if options.shell_mode {
        make_command(&full_command)
    } else {
        make_direct_command(options.command, options.args)
    };

    process.current_dir(&project.root);
    process.env("PATH", path_value);
    process.env("SNPM_PACKAGE_NAME", package_name);

    let status = process.status().map_err(|error| SnpmError::ScriptRun {
        name: options.command.to_string(),
        reason: error.to_string(),
    })?;

    if status.success() {
        Ok(())
    } else {
        Err(SnpmError::ScriptFailed {
            name: options.command.to_string(),
            code: status.code().unwrap_or(1),
        })
    }
}

pub fn exec_workspace_command(
    workspace: &Workspace,
    options: &ExecOptions,
    filters: &[String],
) -> Result<()> {
    for project in &workspace.projects {
        let name = project_label(project);

        if !matches_filters(&name, filters) {
            continue;
        }

        println!("\n{}", name);
        exec_command(project, options)?;
    }

    Ok(())
}

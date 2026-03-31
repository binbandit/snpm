use super::filters::{matches_filters, project_label};
use super::process::{build_path, join_args, make_command};
use crate::{Project, Result, SnpmError, Workspace, console};

pub fn run_script(project: &Project, script: &str, args: &[String]) -> Result<()> {
    let scripts = &project.manifest.scripts;

    if !scripts.contains_key(script) {
        return Err(SnpmError::ScriptMissing {
            name: script.to_string(),
        });
    }

    let pre_name = format!("pre{}", script);
    if scripts.contains_key(&pre_name) {
        run_single_script(project, &pre_name, &[])?;
    }

    run_single_script(project, script, args)?;

    let post_name = format!("post{}", script);
    if scripts.contains_key(&post_name) {
        run_single_script(project, &post_name, &[])?;
    }

    Ok(())
}

pub fn run_workspace_scripts(
    workspace: &Workspace,
    script: &str,
    filters: &[String],
    args: &[String],
) -> Result<()> {
    let mut any_ran = false;

    for project in &workspace.projects {
        let name = project_label(project);

        if !matches_filters(&name, filters) {
            continue;
        }

        if !project.manifest.scripts.contains_key(script) {
            continue;
        }

        any_ran = true;
        println!("\n{}", name);
        run_script(project, script, args)?;
    }

    if !any_ran {
        return Err(SnpmError::ScriptMissing {
            name: script.to_string(),
        });
    }

    Ok(())
}

fn run_single_script(project: &Project, script: &str, args: &[String]) -> Result<()> {
    let scripts = &project.manifest.scripts;
    let base = scripts
        .get(script)
        .ok_or_else(|| SnpmError::ScriptMissing {
            name: script.to_string(),
        })?;

    let mut command_text = base.clone();

    if !args.is_empty() {
        let extra = join_args(args);
        if !command_text.is_empty() {
            command_text.push(' ');
        }
        command_text.push_str(&extra);
    }

    console::info(&command_text);

    let mut command = make_command(&command_text);
    command.current_dir(&project.root);

    let bin_dir = project.root.join("node_modules").join(".bin");
    let path_value = build_path(bin_dir, script)?;
    command.env("PATH", path_value);

    let status = command.status().map_err(|error| SnpmError::ScriptRun {
        name: script.to_string(),
        reason: error.to_string(),
    })?;

    if status.success() {
        Ok(())
    } else {
        let code = status.code().unwrap_or(1);
        Err(SnpmError::ScriptFailed {
            name: script.to_string(),
            code,
        })
    }
}

use super::manifest::{package_name, package_scripts, read_manifest};
use crate::{Result, SnpmConfig, SnpmError, Workspace};

use serde_json::Value;
use std::env;
use std::path::Path;
use std::process::Command;

const LIFECYCLE_SCRIPT_NAMES: [&str; 4] = ["preinstall", "install", "postinstall", "prepare"];

pub fn run_project_scripts(
    _config: &SnpmConfig,
    _workspace: Option<&Workspace>,
    project_root: &Path,
) -> Result<()> {
    let manifest_path = project_root.join("package.json");
    if !manifest_path.exists() {
        return Ok(());
    }

    let value = read_manifest(&manifest_path)?;
    let Some(scripts) = package_scripts(&value) else {
        return Ok(());
    };

    let display_name = package_name(&value)
        .filter(|name| !name.is_empty())
        .unwrap_or("root");
    run_present_scripts(display_name, project_root, scripts)
}

pub(super) fn run_present_scripts(
    package_name: &str,
    root: &Path,
    scripts: &serde_json::Map<String, Value>,
) -> Result<()> {
    for script_name in LIFECYCLE_SCRIPT_NAMES {
        run_script_if_present(package_name, root, scripts, script_name)?;
    }

    Ok(())
}

fn run_script_if_present(
    package_name: &str,
    root: &Path,
    scripts: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<()> {
    let cmd = match scripts.get(key) {
        Some(Value::String(cmd)) if !cmd.is_empty() => cmd.clone(),
        _ => return Ok(()),
    };

    let mut command = make_shell_command(&cmd);
    command.current_dir(root);

    if let Some(existing) = env::var_os("PATH") {
        command.env("PATH", existing);
    }

    let status = command.status().map_err(|error| SnpmError::ScriptRun {
        name: format!("{package_name}:{key}"),
        reason: error.to_string(),
    })?;

    if !status.success() {
        let code = status.code().unwrap_or(1);
        return Err(SnpmError::ScriptFailed {
            name: format!("{package_name}:{key}"),
            code,
        });
    }

    Ok(())
}

#[cfg(unix)]
fn make_shell_command(script: &str) -> Command {
    let mut command = Command::new("sh");
    command.arg("-c").arg(script);
    command
}

#[cfg(windows)]
fn make_shell_command(script: &str) -> Command {
    let mut command = Command::new("cmd");
    command.arg("/C").arg(script);
    command
}

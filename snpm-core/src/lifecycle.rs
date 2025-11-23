use crate::{Result, SnpmConfig, SnpmError, Workspace};
use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::{env, ffi::OsStr};

pub fn run_install_scripts(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    project_root: &Path,
) -> Result<()> {
    let node_modules = project_root.join("node_modules");

    if !node_modules.is_dir() {
        return Ok(());
    }

    walk_node_modules(config, workspace, &node_modules)
}

fn walk_node_modules(config: &SnpmConfig, workspace: Option<&Workspace>, dir: &Path) -> Result<()> {
    for entry in fs::read_dir(dir).map_err(|source| SnpmError::ReadFile {
        path: dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| SnpmError::ReadFile {
            path: dir.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry.file_type().map_err(|source| SnpmError::ReadFile {
            path: path.clone(),
            source,
        })?;

        if !file_type.is_dir() {
            continue;
        }

        if entry.file_name() == OsStr::new(".bin") {
            continue;
        }

        let manifest_path = path.join("package.json");

        if manifest_path.is_file() {
            run_package_scripts(config, workspace, &path, &manifest_path)?;

            let nested = path.join("node_modules");
            if nested.is_dir() {
                walk_node_modules(config, workspace, &nested)?;
            }
        } else {
            // Scoped dirs
            walk_node_modules(config, workspace, &path)?;
        }
    }

    Ok(())
}

fn run_package_scripts(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    pkg_root: &Path,
    manifest_path: &Path,
) -> Result<()> {
    let data = fs::read_to_string(manifest_path).map_err(|source| SnpmError::ReadFile {
        path: manifest_path.to_path_buf(),
        source,
    })?;

    let value: Value = serde_json::from_str(&data).map_err(|source| SnpmError::ParseJson {
        path: manifest_path.to_path_buf(),
        source,
    })?;

    let name = value
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if name.is_empty() {
        return Ok(());
    }

    if !is_dep_script_allowed(config, workspace, &name) {
        return Ok(());
    }

    let scripts = match value.get("scripts") {
        Some(Value::Object(map)) => map,
        _ => return Ok(()),
    };

    run_script_if_present(&name, pkg_root, scripts, "preinstall")?;
    run_script_if_present(&name, pkg_root, scripts, "install")?;
    run_script_if_present(&name, pkg_root, scripts, "postinstall")?;
    run_script_if_present(&name, pkg_root, scripts, "prepare")?;

    Ok(())
}

pub(crate) fn is_dep_script_allowed(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    name: &str,
) -> bool {
    if let Some(ws) = workspace {
        if !ws.config.only_built_dependencies.is_empty() {
            return ws.config.only_built_dependencies.iter().any(|n| n == name);
        }

        if !ws.config.ignored_built_dependencies.is_empty() {
            if ws
                .config
                .ignored_built_dependencies
                .iter()
                .any(|n| n == name)
            {
                return false;
            } else {
                return true;
            }
        }
    }

    if !config.allow_scripts.is_empty() {
        return config.allow_scripts.contains(name);
    }

    false
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

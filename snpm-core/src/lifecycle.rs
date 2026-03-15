use crate::{Result, SnpmConfig, SnpmError, Workspace};
use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, ffi::OsStr};

pub fn run_install_scripts(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    project_root: &Path,
) -> Result<Vec<String>> {
    run_install_scripts_for_projects(config, workspace, &[project_root])
}

pub fn run_install_scripts_for_projects(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    project_roots: &[&Path],
) -> Result<Vec<String>> {
    let mut blocked = Vec::new();
    let mut blocked_seen = BTreeSet::new();
    let mut visited_dirs = BTreeSet::<PathBuf>::new();

    for project_root in project_roots {
        let node_modules = project_root.join("node_modules");

        if node_modules.is_dir() {
            walk_node_modules(
                config,
                workspace,
                &node_modules,
                &mut blocked,
                &mut blocked_seen,
                &mut visited_dirs,
            )?;
        }
    }

    Ok(blocked)
}

pub fn run_project_scripts(
    _config: &SnpmConfig,
    _workspace: Option<&Workspace>,
    project_root: &Path,
) -> Result<()> {
    let manifest_path = project_root.join("package.json");

    if !manifest_path.exists() {
        return Ok(());
    }

    let data = fs::read_to_string(&manifest_path).map_err(|source| SnpmError::ReadFile {
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
        .unwrap_or("root")
        .to_string();

    let scripts = match value.get("scripts") {
        Some(Value::Object(map)) => map,
        _ => return Ok(()),
    };

    // For root project, we run: preinstall, install, postinstall, prepare
    let script_names = ["preinstall", "install", "postinstall", "prepare"];
    let display_name = if name.is_empty() { "root" } else { &name };

    for script_name in script_names {
        run_script_if_present(display_name, project_root, scripts, script_name)?;
    }

    Ok(())
}

fn walk_node_modules(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    dir: &Path,
    blocked: &mut Vec<String>,
    blocked_seen: &mut BTreeSet<String>,
    visited_dirs: &mut BTreeSet<PathBuf>,
) -> Result<()> {
    let scan_dir = fs::canonicalize(dir).unwrap_or_else(|_| dir.to_path_buf());
    if !visited_dirs.insert(scan_dir.clone()) {
        return Ok(());
    }

    for entry in fs::read_dir(&scan_dir).map_err(|source| SnpmError::ReadFile {
        path: scan_dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| SnpmError::ReadFile {
            path: scan_dir.clone(),
            source,
        })?;
        let path = entry.path();

        if !path.is_dir() {
            continue;
        }

        if entry.file_name() == OsStr::new(".bin") {
            continue;
        }

        let visit_path = fs::canonicalize(&path).unwrap_or(path.clone());
        let manifest_path = visit_path.join("package.json");

        if manifest_path.is_file() {
            if !visited_dirs.insert(visit_path.clone()) {
                continue;
            }

            run_package_scripts(
                config,
                workspace,
                &visit_path,
                &manifest_path,
                blocked,
                blocked_seen,
            )?;

            let nested = visit_path.join("node_modules");
            if nested.is_dir() {
                walk_node_modules(
                    config,
                    workspace,
                    &nested,
                    blocked,
                    blocked_seen,
                    visited_dirs,
                )?;
            }
        } else {
            walk_node_modules(
                config,
                workspace,
                &visit_path,
                blocked,
                blocked_seen,
                visited_dirs,
            )?;
        }
    }

    Ok(())
}

fn run_package_scripts(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    pkg_root: &Path,
    manifest_path: &Path,
    blocked: &mut Vec<String>,
    blocked_seen: &mut BTreeSet<String>,
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

    let scripts = match value.get("scripts") {
        Some(Value::Object(map)) => map,
        _ => return Ok(()),
    };

    let script_names = ["preinstall", "install", "postinstall", "prepare"];
    let has_any_script = script_names.iter().any(|s| scripts.contains_key(*s));

    if !has_any_script {
        return Ok(());
    }

    if !is_dep_script_allowed(config, workspace, &name) {
        if blocked_seen.insert(name.clone()) {
            blocked.push(name);
        }
        return Ok(());
    }

    for script_name in script_names {
        run_script_if_present(&name, pkg_root, scripts, script_name)?;
    }

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
            return !ws
                .config
                .ignored_built_dependencies
                .iter()
                .any(|n| n == name);
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

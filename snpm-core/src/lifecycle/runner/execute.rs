use super::manifest::{package_name, package_scripts, read_manifest};
use crate::{Result, SnpmConfig, SnpmError, Workspace};

use serde_json::Value;
use std::env;
use std::ffi::OsString;
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
    run_present_scripts(display_name, project_root, scripts)?;
    Ok(())
}

pub(super) fn run_present_scripts(
    package_name: &str,
    root: &Path,
    scripts: &serde_json::Map<String, Value>,
) -> Result<usize> {
    let mut ran = 0;

    for script_name in LIFECYCLE_SCRIPT_NAMES {
        ran += usize::from(run_script_if_present(
            package_name,
            root,
            scripts,
            script_name,
        )?);
    }

    Ok(ran)
}

fn run_script_if_present(
    package_name: &str,
    root: &Path,
    scripts: &serde_json::Map<String, Value>,
    key: &str,
) -> Result<bool> {
    let cmd = match scripts.get(key) {
        Some(Value::String(cmd)) if !cmd.is_empty() => cmd.clone(),
        _ => return Ok(false),
    };

    let mut command = make_shell_command(&cmd);
    command.current_dir(root);
    let path_value = build_path(root, &format!("{package_name}:{key}"))?;
    command.env("PATH", path_value);

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

    Ok(true)
}

fn build_path(root: &Path, script_name: &str) -> Result<OsString> {
    let mut parts = vec![root.join("node_modules").join(".bin")];

    if let Some(existing) = env::var_os("PATH") {
        for path in env::split_paths(&existing) {
            parts.push(path);
        }
    }

    env::join_paths(parts).map_err(|error| SnpmError::ScriptRun {
        name: script_name.to_string(),
        reason: error.to_string(),
    })
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

#[cfg(test)]
mod tests {
    use super::run_present_scripts;

    use serde_json::{Map, Value};
    use std::fs;
    use std::path::Path;
    use tempfile::tempdir;

    fn write_bin_command(root: &Path, name: &str, marker_name: &str) {
        let bin_dir = root.join("node_modules").join(".bin");
        fs::create_dir_all(&bin_dir).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let script_path = bin_dir.join(name);
            fs::write(
                &script_path,
                format!("#!/bin/sh\necho ok > {marker_name}\n"),
            )
            .unwrap();

            let mut permissions = fs::metadata(&script_path).unwrap().permissions();
            permissions.set_mode(0o755);
            fs::set_permissions(&script_path, permissions).unwrap();
        }

        #[cfg(windows)]
        {
            let script_path = bin_dir.join(format!("{name}.cmd"));
            fs::write(
                &script_path,
                format!("@echo off\r\necho ok > {marker_name}\r\n"),
            )
            .unwrap();
        }
    }

    #[test]
    fn run_present_scripts_uses_local_node_modules_bin() {
        let dir = tempdir().unwrap();
        let marker = dir.path().join("prepare-marker.txt");

        write_bin_command(dir.path(), "effect-language-service", "prepare-marker.txt");

        let mut scripts = Map::new();
        scripts.insert(
            "prepare".to_string(),
            Value::String("effect-language-service".to_string()),
        );

        let ran = run_present_scripts("pkg", dir.path(), &scripts).unwrap();

        assert!(marker.is_file());
        assert_eq!(ran, 1);
    }
}

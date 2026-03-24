use crate::{Result, SnpmConfig, SnpmError, Workspace, console};
use std::fs;
use std::path::Path;
use std::process::Command;

pub fn rebuild(config: &SnpmConfig, workspace: Option<&Workspace>, root: &Path) -> Result<usize> {
    let node_modules = root.join("node_modules");
    let virtual_store = node_modules.join(".snpm");

    if !virtual_store.is_dir() {
        return Ok(0);
    }

    let allowed = &config.allow_scripts;
    let ws_only = workspace
        .map(|w| &w.config.only_built_dependencies)
        .cloned()
        .unwrap_or_default();

    let mut rebuilt = 0;

    let entries = fs::read_dir(&virtual_store).map_err(|source| SnpmError::ReadFile {
        path: virtual_store.clone(),
        source,
    })?;

    for entry in entries.flatten() {
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }

        let nm_dir = entry_path.join("node_modules");
        if !nm_dir.is_dir() {
            continue;
        }

        // Find the package directory inside node_modules
        for pkg_entry in fs::read_dir(&nm_dir).into_iter().flatten().flatten() {
            let pkg_path = pkg_entry.path();
            if !pkg_path.is_dir() {
                continue;
            }

            let pkg_json = pkg_path.join("package.json");
            if !pkg_json.is_file() {
                continue;
            }

            let data = match fs::read_to_string(&pkg_json) {
                Ok(d) => d,
                Err(_) => continue,
            };
            let manifest: serde_json::Value = match serde_json::from_str(&data) {
                Ok(v) => v,
                Err(_) => continue,
            };

            let scripts = match manifest.get("scripts").and_then(|s| s.as_object()) {
                Some(s) => s,
                None => continue,
            };

            let name = manifest
                .get("name")
                .and_then(|n| n.as_str())
                .unwrap_or("");

            // Check if this package is allowed to run scripts
            let can_run = allowed.iter().any(|a| a == name || a == "*")
                || ws_only.iter().any(|d| d == name);

            if !can_run {
                continue;
            }

            let has_install_script = scripts.contains_key("preinstall")
                || scripts.contains_key("install")
                || scripts.contains_key("postinstall");

            if !has_install_script {
                continue;
            }

            console::step(&format!("Rebuilding {}", name));

            for script_name in &["preinstall", "install", "postinstall"] {
                if let Some(cmd) = scripts.get(*script_name).and_then(|v| v.as_str()) {
                    run_script(&pkg_path, script_name, cmd)?;
                }
            }

            rebuilt += 1;
        }
    }

    Ok(rebuilt)
}

fn run_script(cwd: &Path, name: &str, cmd: &str) -> Result<()> {
    console::verbose(&format!("running {} in {}: {}", name, cwd.display(), cmd));

    let shell = if cfg!(windows) { "cmd" } else { "sh" };
    let flag = if cfg!(windows) { "/C" } else { "-c" };

    let status = Command::new(shell)
        .arg(flag)
        .arg(cmd)
        .current_dir(cwd)
        .status()
        .map_err(|source| SnpmError::ScriptRun {
            name: name.to_string(),
            reason: source.to_string(),
        })?;

    if !status.success() {
        return Err(SnpmError::ScriptFailed {
            name: name.to_string(),
            code: status.code().unwrap_or(-1),
        });
    }

    Ok(())
}

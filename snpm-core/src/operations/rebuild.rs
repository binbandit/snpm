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
    let workspace_only_built = workspace
        .map(|workspace| &workspace.config.only_built_dependencies)
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

        let node_modules_dir = entry_path.join("node_modules");
        if !node_modules_dir.is_dir() {
            continue;
        }

        for package_entry in fs::read_dir(&node_modules_dir).into_iter().flatten().flatten() {
            let package_path = package_entry.path();
            if !package_path.is_dir() {
                continue;
            }

            let manifest_path = package_path.join("package.json");
            if !manifest_path.is_file() {
                continue;
            }

            let content = match fs::read_to_string(&manifest_path) {
                Ok(content) => content,
                Err(_) => continue,
            };
            let manifest: serde_json::Value = match serde_json::from_str(&content) {
                Ok(value) => value,
                Err(_) => continue,
            };

            let scripts = match manifest.get("scripts").and_then(|scripts| scripts.as_object()) {
                Some(scripts) => scripts,
                None => continue,
            };

            let name = manifest
                .get("name")
                .and_then(|name| name.as_str())
                .unwrap_or("");

            let is_allowed = allowed.iter().any(|allowed_name| allowed_name == name || allowed_name == "*")
                || workspace_only_built.iter().any(|allowed_dep| allowed_dep == name);

            if !is_allowed {
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
                if let Some(command) = scripts.get(*script_name).and_then(|value| value.as_str()) {
                    run_script(&package_path, script_name, command)?;
                }
            }

            rebuilt += 1;
        }
    }

    Ok(rebuilt)
}

fn run_script(working_directory: &Path, name: &str, command: &str) -> Result<()> {
    console::verbose(&format!("{} in {}: {}", name, working_directory.display(), command));

    let shell = if cfg!(windows) { "cmd" } else { "sh" };
    let shell_flag = if cfg!(windows) { "/C" } else { "-c" };

    let status = Command::new(shell)
        .arg(shell_flag)
        .arg(command)
        .current_dir(working_directory)
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

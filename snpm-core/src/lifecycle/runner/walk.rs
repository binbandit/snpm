use super::super::policy::is_dep_script_allowed;
use super::execute::run_present_scripts;
use super::manifest::{package_name, package_scripts, read_manifest};
use crate::{Result, SnpmConfig, SnpmError, Workspace};

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

const LIFECYCLE_SCRIPT_NAMES: [&str; 4] = ["preinstall", "install", "postinstall", "prepare"];

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
    let value = read_manifest(manifest_path)?;
    let Some(name) = package_name(&value).filter(|name| !name.is_empty()) else {
        return Ok(());
    };
    let Some(scripts) = package_scripts(&value) else {
        return Ok(());
    };

    if !has_lifecycle_scripts(scripts) {
        return Ok(());
    }

    if !is_dep_script_allowed(config, workspace, name) {
        if blocked_seen.insert(name.to_string()) {
            blocked.push(name.to_string());
        }
        return Ok(());
    }

    run_present_scripts(name, pkg_root, scripts)
}

fn has_lifecycle_scripts(scripts: &serde_json::Map<String, serde_json::Value>) -> bool {
    LIFECYCLE_SCRIPT_NAMES
        .iter()
        .any(|script_name| scripts.contains_key(*script_name))
}

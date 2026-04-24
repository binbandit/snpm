use super::super::policy::is_dep_script_allowed;
use super::cache::{SideEffectsCacheEntry, SideEffectsCacheRestore};
use super::execute::run_present_scripts;
use super::manifest::{package_name, package_scripts, package_version, read_manifest};
use crate::console;
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

    let cache_entry = package_version(&value)
        .filter(|version| !version.is_empty())
        .map(|version| SideEffectsCacheEntry::new(config, name, version, pkg_root))
        .transpose()?;

    if let Some(cache_entry) = cache_entry.as_ref() {
        match cache_entry.restore_if_available(pkg_root) {
            Ok(SideEffectsCacheRestore::Restored | SideEffectsCacheRestore::AlreadyApplied) => {
                return Ok(());
            }
            Ok(SideEffectsCacheRestore::Miss) => {}
            Err(error) => {
                console::warn(&format!(
                    "failed to restore side-effects cache for {}: {}",
                    name, error
                ));
            }
        }
    }

    let ran = run_present_scripts(name, pkg_root, scripts)?;

    if ran > 0
        && let Some(cache_entry) = cache_entry
        && let Err(error) = cache_entry.save(pkg_root)
    {
        console::warn(&format!(
            "failed to save side-effects cache for {}: {}",
            name, error
        ));
    }

    Ok(())
}

fn has_lifecycle_scripts(scripts: &serde_json::Map<String, serde_json::Value>) -> bool {
    LIFECYCLE_SCRIPT_NAMES
        .iter()
        .any(|script_name| scripts.contains_key(*script_name))
}

#[cfg(test)]
mod tests {
    use super::run_install_scripts;
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};

    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_config(data_dir: PathBuf) -> SnpmConfig {
        SnpmConfig {
            cache_dir: data_dir.join("cache"),
            data_dir,
            allow_scripts: BTreeSet::from(["dep".to_string()]),
            disable_global_virtual_store_for_packages: BTreeSet::new(),
            min_package_age_days: None,
            min_package_cache_age_days: None,
            default_registry: "https://registry.npmjs.org".to_string(),
            scoped_registries: BTreeMap::new(),
            registry_auth: BTreeMap::new(),
            default_registry_auth_token: None,
            default_registry_auth_scheme: AuthScheme::Bearer,
            registry_auth_schemes: BTreeMap::new(),
            hoisting: HoistingMode::SingleVersion,
            link_backend: LinkBackend::Auto,
            strict_peers: false,
            frozen_lockfile_default: false,
            always_auth: false,
            registry_concurrency: 64,
            verbose: false,
            log_file: None,
        }
    }

    #[cfg(unix)]
    #[test]
    fn run_install_scripts_restores_cached_side_effects() {
        let dir = tempdir().unwrap();
        let project_root = dir.path();
        let dep_root = project_root.join("node_modules").join("dep");
        let counter = project_root.join("side-effects-counter.txt");
        let built = dep_root.join("built.txt");
        let config = make_config(project_root.join(".snpm-data"));

        fs::create_dir_all(&dep_root).unwrap();
        fs::write(
            dep_root.join("package.json"),
            format!(
                r#"{{
  "name": "dep",
  "version": "1.0.0",
  "scripts": {{
    "postinstall": "echo run >> '{}' && echo built > built.txt"
  }}
}}
"#,
                counter.display()
            ),
        )
        .unwrap();

        run_install_scripts(&config, None, project_root).unwrap();
        assert_eq!(fs::read_to_string(&counter).unwrap().lines().count(), 1);
        assert_eq!(fs::read_to_string(&built).unwrap(), "built\n");

        fs::remove_dir_all(&dep_root).unwrap();
        fs::create_dir_all(&dep_root).unwrap();
        fs::write(
            dep_root.join("package.json"),
            format!(
                r#"{{
  "name": "dep",
  "version": "1.0.0",
  "scripts": {{
    "postinstall": "echo run >> '{}' && echo built > built.txt"
  }}
}}
"#,
                counter.display()
            ),
        )
        .unwrap();

        run_install_scripts(&config, None, project_root).unwrap();

        assert_eq!(fs::read_to_string(&counter).unwrap().lines().count(), 1);
        assert_eq!(fs::read_to_string(&built).unwrap(), "built\n");
    }
}

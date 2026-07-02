use crate::{Project, Result, SnpmConfig, SnpmError};

use std::fs;
use std::path::Path;

/// The global install space is an ordinary snpm project living at
/// `config.global_dir()`: a managed `package.json` whose dependencies
/// are the globally installed packages, plus the usual
/// `snpm-lock.yaml` / `node_modules` / `.snpm` produced by the standard
/// install pipeline. This gives global installs the full resolver,
/// store, virtual-store linking, and hot-path behavior for free —
/// including transitive dependencies, which a package copied in
/// isolation would be missing at runtime.
const GLOBAL_MANIFEST_TEMPLATE: &str =
    "{\n  \"name\": \"snpm-global\",\n  \"private\": true,\n  \"dependencies\": {}\n}\n";

/// Entries the managed project owns; everything else in the global dir
/// is a package installed by the pre-project layout.
const MANAGED_GLOBAL_ENTRIES: &[&str] = &[
    "package.json",
    "node_modules",
    "snpm-lock.yaml",
    "snpm-lock.bin",
];

pub(super) fn global_project(config: &SnpmConfig) -> Result<Project> {
    let global_dir = config.global_dir();
    fs::create_dir_all(&global_dir).map_err(|source| SnpmError::WriteFile {
        path: global_dir.clone(),
        source,
    })?;

    let manifest_path = global_dir.join("package.json");
    if !manifest_path.is_file() {
        fs::write(&manifest_path, GLOBAL_MANIFEST_TEMPLATE).map_err(|source| {
            SnpmError::WriteFile {
                path: manifest_path.clone(),
                source,
            }
        })?;
    }

    Project::from_manifest_path(manifest_path)
}

/// Fold packages installed by the pre-project global layout (each one a
/// bare `<global>/<name>` copy with no dependency tree) into the managed
/// manifest, then delete the legacy copies. The caller's subsequent
/// install resolves the migrated packages with their full trees and
/// re-links their launchers; until then the old launchers dangle, which
/// the standard dangling-launcher pruning cleans up.
pub(super) fn migrate_legacy_global_packages(project: &mut Project) -> Result<Vec<String>> {
    let mut migrated = Vec::new();

    for legacy_dir in legacy_package_dirs(&project.root) {
        let Some((name, version)) = read_legacy_identity(&legacy_dir) else {
            continue;
        };

        project
            .manifest
            .dependencies
            .entry(name.clone())
            .or_insert_with(|| match &version {
                Some(version) => format!("^{version}"),
                None => "*".to_string(),
            });

        fs::remove_dir_all(&legacy_dir).map_err(|source| SnpmError::WriteFile {
            path: legacy_dir.clone(),
            source,
        })?;
        migrated.push(name);
    }

    if !migrated.is_empty() {
        crate::console::info(&format!(
            "migrating {} to the managed global project",
            migrated.join(", ")
        ));
        project.write_manifest(&project.manifest)?;
    }

    Ok(migrated)
}

/// Read-only view of packages still in the pre-project layout, for
/// `snpm list -g`: (name, version) pairs. They are folded into the
/// managed project by the next `snpm add -g` / `snpm remove -g`.
pub fn legacy_global_packages(config: &SnpmConfig) -> Vec<(String, Option<String>)> {
    legacy_package_dirs(&config.global_dir())
        .iter()
        .filter_map(|dir| read_legacy_identity(dir))
        .collect()
}

fn legacy_package_dirs(global_root: &Path) -> Vec<std::path::PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(global_root) else {
        return found;
    };

    for entry in entries.flatten() {
        let file_name = entry.file_name().to_string_lossy().into_owned();
        if MANAGED_GLOBAL_ENTRIES.contains(&file_name.as_str())
            || file_name.starts_with('.')
            || !entry.path().is_dir()
        {
            continue;
        }

        if file_name.starts_with('@') {
            // Old layout nested scoped packages as `<global>/@scope/<name>`.
            if let Ok(scoped) = fs::read_dir(entry.path()) {
                for scoped_entry in scoped.flatten() {
                    if is_legacy_package_dir(&scoped_entry.path()) {
                        found.push(scoped_entry.path());
                    }
                }
            }
            continue;
        }

        if is_legacy_package_dir(&entry.path()) {
            found.push(entry.path());
        }
    }

    found
}

fn is_legacy_package_dir(path: &Path) -> bool {
    path.is_dir() && path.join("package.json").is_file()
}

fn read_legacy_identity(package_dir: &Path) -> Option<(String, Option<String>)> {
    let content = fs::read_to_string(package_dir.join("package.json")).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&content).ok()?;

    let name = manifest
        .get("name")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
        .or_else(|| dir_package_name(package_dir))?;
    let version = manifest
        .get("version")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string);

    Some((name, version))
}

/// `<global>/@scope/name` → `@scope/name`, `<global>/name` → `name`.
fn dir_package_name(package_dir: &Path) -> Option<String> {
    let name = package_dir.file_name()?.to_string_lossy().into_owned();
    let parent = package_dir.parent()?.file_name()?.to_string_lossy();
    if parent.starts_with('@') {
        Some(format!("{parent}/{name}"))
    } else {
        Some(name)
    }
}

use crate::lockfile;
use crate::resolve::RootDependency;
use crate::{Project, Result, Workspace};

use std::collections::{BTreeMap, BTreeSet};

use super::super::super::utils::OutdatedEntry;

pub(super) fn read_current_versions(
    project: &Project,
    workspace: Option<&Workspace>,
) -> Result<BTreeMap<String, String>> {
    let lockfile_path = workspace
        .map(|workspace| workspace.root.join("snpm-lock.yaml"))
        .unwrap_or_else(|| project.root.join("snpm-lock.yaml"));

    let mut current_versions = BTreeMap::new();
    if lockfile_path.is_file() {
        let existing = lockfile::read(&lockfile_path)?;
        for (name, dep) in &existing.root.dependencies {
            if let Some(version) = dep.version.as_ref() {
                current_versions.insert(name.clone(), version.clone());
            }
        }
    }

    Ok(current_versions)
}

pub(super) fn build_outdated_entries(
    include_dev: bool,
    dependencies: &BTreeMap<String, String>,
    development_dependencies: &BTreeMap<String, String>,
    current_versions: &BTreeMap<String, String>,
    resolved_roots: &BTreeMap<String, RootDependency>,
) -> Vec<OutdatedEntry> {
    let names = collect_manifest_names(include_dev, dependencies, development_dependencies);
    let mut result = Vec::new();

    for name in names {
        let Some(root_dep) = resolved_roots.get(&name) else {
            continue;
        };

        let wanted = root_dep.resolved.version.clone();
        let current = current_versions.get(&name).cloned();

        if current
            .as_ref()
            .is_some_and(|current_version| current_version == &wanted)
        {
            continue;
        }

        result.push(OutdatedEntry {
            name,
            current,
            wanted,
        });
    }

    result
}

fn collect_manifest_names(
    include_dev: bool,
    dependencies: &BTreeMap<String, String>,
    development_dependencies: &BTreeMap<String, String>,
) -> BTreeSet<String> {
    let mut names = BTreeSet::new();

    for name in dependencies.keys() {
        names.insert(name.clone());
    }

    if include_dev {
        for name in development_dependencies.keys() {
            names.insert(name.clone());
        }
    }

    names
}

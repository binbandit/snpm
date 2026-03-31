use crate::registry::RegistryProtocol;
use crate::{Result, SnpmConfig, SnpmError, Workspace};

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use super::super::manifest::{RootSpecSet, detect_manifest_protocol};
use super::root_specs::collect_workspace_root_specs;

pub(super) struct WorkspaceInstallSetup {
    pub(super) lockfile_path: PathBuf,
    pub(super) root_specs: RootSpecSet,
    pub(super) root_dependencies: BTreeMap<String, String>,
    pub(super) root_protocols: BTreeMap<String, RegistryProtocol>,
    pub(super) optional_root_names: BTreeSet<String>,
}

pub(super) fn prepare_workspace_install(
    config: &SnpmConfig,
    workspace: &Workspace,
    include_dev: bool,
    frozen_lockfile: bool,
) -> Result<WorkspaceInstallSetup> {
    let lockfile_path = workspace.root.join("snpm-lock.yaml");
    if (frozen_lockfile || config.frozen_lockfile_default) && !lockfile_path.is_file() {
        return Err(SnpmError::Lockfile {
            path: lockfile_path,
            reason: "frozen-lockfile requested but snpm-lock.yaml is missing".into(),
        });
    }

    let root_specs = collect_workspace_root_specs(workspace, include_dev)?;
    let mut root_dependencies = root_specs.required.clone();
    for (name, range) in &root_specs.optional {
        root_dependencies.insert(name.clone(), range.clone());
    }

    Ok(WorkspaceInstallSetup {
        optional_root_names: root_specs.optional.keys().cloned().collect(),
        root_protocols: build_root_protocols(&root_dependencies),
        root_dependencies,
        root_specs,
        lockfile_path,
    })
}

fn build_root_protocols(
    root_dependencies: &BTreeMap<String, String>,
) -> BTreeMap<String, RegistryProtocol> {
    root_dependencies
        .iter()
        .map(|(name, spec)| {
            let protocol = detect_manifest_protocol(spec).unwrap_or_else(RegistryProtocol::npm);
            (name.clone(), protocol)
        })
        .collect()
}

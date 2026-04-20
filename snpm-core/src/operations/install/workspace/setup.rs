use crate::lockfile::CompatibleLockfile;
use crate::operations::install::utils::FrozenLockfileMode;
use crate::registry::RegistryProtocol;
use crate::{Result, SnpmError, Workspace};

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use super::super::manifest::{RootSpecSet, detect_manifest_protocol};
use super::root_specs::collect_workspace_root_specs;

pub(super) struct WorkspaceInstallSetup {
    pub(super) lockfile_path: PathBuf,
    pub(super) compatible_lockfile: Option<CompatibleLockfile>,
    pub(super) root_specs: RootSpecSet,
    pub(super) root_dependencies: BTreeMap<String, String>,
    pub(super) root_protocols: BTreeMap<String, RegistryProtocol>,
    pub(super) optional_root_names: BTreeSet<String>,
}

pub(super) fn prepare_workspace_install(
    workspace: &Workspace,
    include_dev: bool,
    frozen_lockfile: FrozenLockfileMode,
    strict_no_lockfile: bool,
) -> Result<WorkspaceInstallSetup> {
    let lockfile_path = workspace.root.join("snpm-lock.yaml");
    let compatible_lockfile = if !lockfile_path.is_file() {
        crate::lockfile::detect_compatible_lockfile(&workspace.root)
    } else {
        None
    };
    if strict_no_lockfile
        && matches!(frozen_lockfile, FrozenLockfileMode::Frozen)
        && !lockfile_path.is_file()
        && compatible_lockfile.is_none()
    {
        return Err(SnpmError::Lockfile {
            path: lockfile_path,
            reason: "frozen-lockfile requested but neither snpm-lock.yaml nor a supported compatible lockfile was found".into(),
        });
    }

    let root_specs = collect_workspace_root_specs(workspace, include_dev)?;
    let mut root_dependencies = root_specs.required.clone();
    for (name, range) in &root_specs.optional {
        root_dependencies.insert(name.clone(), range.clone());
    }

    Ok(WorkspaceInstallSetup {
        compatible_lockfile,
        optional_root_names: root_specs.optional.keys().cloned().collect(),
        root_protocols: build_root_protocols(&root_dependencies),
        root_dependencies,
        root_specs,
        lockfile_path,
    })
}

impl WorkspaceInstallSetup {
    pub(super) fn lockfile_source_path(&self) -> PathBuf {
        if self.lockfile_path.is_file() {
            return self.lockfile_path.clone();
        }

        self.compatible_lockfile
            .as_ref()
            .map(|source| source.path.clone())
            .unwrap_or_else(|| self.lockfile_path.clone())
    }

    pub(super) fn has_compatible_lockfile(&self) -> bool {
        self.compatible_lockfile.is_some()
    }
}

pub(super) fn build_root_protocols(
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

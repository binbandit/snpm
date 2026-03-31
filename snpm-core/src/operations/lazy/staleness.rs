use crate::lockfile;
use crate::{Project, Workspace};

use std::fs;

use super::super::install::{build_project_integrity_state, check_integrity_file};
use super::manifest::build_manifest_root;

pub(super) enum StalenessReason {
    NoLockfile,
    NoNodeModules,
    NoIntegrityFile,
    IntegrityMismatch,
    ManifestChanged,
}

pub(super) struct StalenessCheck {
    pub(super) is_stale: bool,
    pub(super) reason: Option<StalenessReason>,
}

pub(super) fn check_staleness(project: &Project, workspace: Option<&Workspace>) -> StalenessCheck {
    let lockfile_path = workspace
        .map(|workspace| workspace.root.join("snpm-lock.yaml"))
        .unwrap_or_else(|| project.root.join("snpm-lock.yaml"));

    if !lockfile_path.is_file() {
        return stale(StalenessReason::NoLockfile);
    }

    let node_modules = project.root.join("node_modules");
    if !node_modules.is_dir() {
        return stale(StalenessReason::NoNodeModules);
    }

    let integrity_path = node_modules.join(".snpm-integrity");
    if fs::read_to_string(&integrity_path).is_err() {
        return stale(StalenessReason::NoIntegrityFile);
    }

    let existing_lockfile = match lockfile::read(&lockfile_path) {
        Ok(lockfile) => lockfile,
        Err(_) => return stale(StalenessReason::NoLockfile),
    };

    let manifest_specs = match build_manifest_root(project, workspace) {
        Ok(root) => root,
        Err(_) => return stale(StalenessReason::ManifestChanged),
    };

    if !lockfile::root_specs_match(
        &existing_lockfile,
        &manifest_specs.required,
        &manifest_specs.optional,
    ) {
        return stale(StalenessReason::ManifestChanged);
    }

    let graph = lockfile::to_graph(&existing_lockfile);
    let integrity_state = match build_project_integrity_state(project, &graph) {
        Ok(state) => state,
        Err(_) => return stale(StalenessReason::IntegrityMismatch),
    };

    if !check_integrity_file(project, &integrity_state) {
        return stale(StalenessReason::IntegrityMismatch);
    }

    StalenessCheck {
        is_stale: false,
        reason: None,
    }
}

fn stale(reason: StalenessReason) -> StalenessCheck {
    StalenessCheck {
        is_stale: true,
        reason: Some(reason),
    }
}

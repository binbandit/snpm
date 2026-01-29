use crate::lockfile;
use crate::workspace::CatalogConfig;
use crate::{Project, Result, SnpmConfig, Workspace, console};
use std::collections::BTreeSet;
use std::fs;

use super::install::{
    InstallOptions, apply_specs, build_project_manifest_root, collect_workspace_root_deps,
};

pub enum StalenessReason {
    NoLockfile,
    NoNodeModules,
    NoIntegrityFile,
    IntegrityMismatch,
    ManifestChanged,
}

pub struct StalenessCheck {
    pub is_stale: bool,
    pub reason: Option<StalenessReason>,
}

pub fn check_staleness(project: &Project, workspace: Option<&Workspace>) -> StalenessCheck {
    let lockfile_path = workspace
        .map(|w| w.root.join("snpm-lock.yaml"))
        .unwrap_or_else(|| project.root.join("snpm-lock.yaml"));

    if !lockfile_path.is_file() {
        return StalenessCheck {
            is_stale: true,
            reason: Some(StalenessReason::NoLockfile),
        };
    }

    let node_modules = project.root.join("node_modules");
    if !node_modules.is_dir() {
        return StalenessCheck {
            is_stale: true,
            reason: Some(StalenessReason::NoNodeModules),
        };
    }

    let integrity_path = node_modules.join(".snpm-integrity");
    let integrity_content = match fs::read_to_string(&integrity_path) {
        Ok(content) => content,
        Err(_) => {
            return StalenessCheck {
                is_stale: true,
                reason: Some(StalenessReason::NoIntegrityFile),
            };
        }
    };

    let existing_lockfile = match lockfile::read(&lockfile_path) {
        Ok(lockfile) => lockfile,
        Err(_) => {
            return StalenessCheck {
                is_stale: true,
                reason: Some(StalenessReason::NoLockfile),
            };
        }
    };

    let manifest_root = match build_manifest_root(project, workspace) {
        Ok(root) => root,
        Err(_) => {
            return StalenessCheck {
                is_stale: true,
                reason: Some(StalenessReason::ManifestChanged),
            };
        }
    };

    let mut lock_requested = std::collections::BTreeMap::new();
    for (name, dep) in existing_lockfile.root.dependencies.iter() {
        lock_requested.insert(name.clone(), dep.requested.clone());
    }

    if lock_requested != manifest_root {
        return StalenessCheck {
            is_stale: true,
            reason: Some(StalenessReason::ManifestChanged),
        };
    }

    let graph = lockfile::to_graph(&existing_lockfile);
    let lockfile_hash = super::install::compute_lockfile_hash(&graph);
    let expected = format!("lockfile: {}\n", lockfile_hash);

    if integrity_content != expected {
        return StalenessCheck {
            is_stale: true,
            reason: Some(StalenessReason::IntegrityMismatch),
        };
    }

    StalenessCheck {
        is_stale: false,
        reason: None,
    }
}

fn build_manifest_root(
    project: &Project,
    workspace: Option<&Workspace>,
) -> Result<std::collections::BTreeMap<String, String>> {
    let catalog = if workspace.is_none() {
        CatalogConfig::load(&project.root)?
    } else {
        None
    };

    let mut local_deps = BTreeSet::new();
    let mut local_dev_deps = BTreeSet::new();

    let dependencies = apply_specs(
        &project.manifest.dependencies,
        workspace,
        catalog.as_ref(),
        &mut local_deps,
        None,
    )?;

    let development_dependencies = apply_specs(
        &project.manifest.dev_dependencies,
        workspace,
        catalog.as_ref(),
        &mut local_dev_deps,
        None,
    )?;

    if let Some(ws) = workspace {
        collect_workspace_root_deps(ws, true)
    } else {
        Ok(build_project_manifest_root(
            &dependencies,
            &development_dependencies,
            true,
        ))
    }
}

pub async fn lazy_install(config: &SnpmConfig, project: &mut Project) -> Result<()> {
    let workspace = Workspace::discover(&project.root)?;
    let check = check_staleness(project, workspace.as_ref());

    if !check.is_stale {
        return Ok(());
    }

    let reason_message = match check.reason {
        Some(StalenessReason::NoLockfile) => "lockfile missing",
        Some(StalenessReason::NoNodeModules) => "node_modules missing",
        Some(StalenessReason::NoIntegrityFile) => "integrity file missing",
        Some(StalenessReason::IntegrityMismatch) => "integrity mismatch",
        Some(StalenessReason::ManifestChanged) => "manifest changed",
        None => "unknown",
    };

    console::info(&format!("Installing dependencies ({})...", reason_message));

    let options = InstallOptions {
        requested: Vec::new(),
        dev: false,
        include_dev: true,
        frozen_lockfile: false,
        force: false,
        silent_summary: true,
    };

    super::install(config, project, options).await?;

    Ok(())
}

pub fn is_stale(project: &Project) -> bool {
    let workspace = match Workspace::discover(&project.root) {
        Ok(ws) => ws,
        Err(_) => None,
    };

    check_staleness(project, workspace.as_ref()).is_stale
}

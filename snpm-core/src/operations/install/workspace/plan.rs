use crate::lockfile;
use crate::operations::install::utils::FrozenLockfileMode;
use crate::{Result, SnpmConfig, Workspace};
use std::collections::BTreeMap;

use super::super::utils::InstallScenario;
use super::resolution::{detect_workspace_scenario_early, validate_lockfile_matches_manifest};
use super::setup::{WorkspaceInstallSetup, build_root_protocols, prepare_workspace_install};

pub(super) struct WorkspaceInstallPlan {
    pub(super) setup: WorkspaceInstallSetup,
    pub(super) scenario: InstallScenario,
    pub(super) existing_lockfile: Option<lockfile::Lockfile>,
}

fn pinned_workspace_root_dependencies_for_fix(
    setup: &WorkspaceInstallSetup,
    existing_lockfile: Option<&lockfile::Lockfile>,
) -> BTreeMap<String, String> {
    let mut root_dependencies = setup.root_dependencies.clone();
    let Some(existing) = existing_lockfile else {
        return root_dependencies;
    };

    for (name, requested) in &setup.root_specs.required {
        let Some(dep) = existing.root.dependencies.get(name) else {
            continue;
        };

        if dep.optional || dep.version.is_none() || dep.requested != *requested {
            continue;
        }

        if let Some(version) = dep.version.as_ref() {
            root_dependencies.insert(name.clone(), version.clone());
        }
    }

    for (name, requested) in &setup.root_specs.optional {
        let Some(dep) = existing.root.dependencies.get(name) else {
            continue;
        };

        if dep.requested != *requested || dep.version.is_none() {
            continue;
        }

        if let Some(version) = dep.version.as_ref() {
            root_dependencies.insert(name.clone(), version.clone());
        }
    }

    root_dependencies
}

pub(super) fn plan_workspace_install(
    config: &SnpmConfig,
    workspace: &Workspace,
    include_dev: bool,
    frozen_lockfile: FrozenLockfileMode,
    strict_no_lockfile: bool,
    force: bool,
) -> Result<WorkspaceInstallPlan> {
    let mut setup =
        prepare_workspace_install(workspace, include_dev, frozen_lockfile, strict_no_lockfile)?;
    let lockfile_source_path = setup.lockfile_source_path();
    let (scenario, existing_lockfile) = detect_workspace_scenario_early(
        workspace,
        &setup.lockfile_path,
        setup.compatible_lockfile.as_ref(),
        config,
        frozen_lockfile,
        strict_no_lockfile,
        force,
    );
    let (scenario, existing_lockfile) = validate_lockfile_matches_manifest(
        frozen_lockfile,
        &lockfile_source_path,
        strict_no_lockfile,
        scenario,
        existing_lockfile,
        &setup.root_specs.required,
        &setup.root_specs.optional,
    )?;

    if matches!(frozen_lockfile, FrozenLockfileMode::Fix) {
        setup.root_dependencies =
            pinned_workspace_root_dependencies_for_fix(&setup, existing_lockfile.as_ref());
        setup.root_protocols = build_root_protocols(&setup.root_dependencies);
    }

    Ok(WorkspaceInstallPlan {
        setup,
        scenario,
        existing_lockfile,
    })
}

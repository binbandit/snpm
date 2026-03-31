use crate::lockfile;
use crate::{Result, SnpmConfig, Workspace};

use super::super::utils::InstallScenario;
use super::resolution::{detect_workspace_scenario_early, validate_lockfile_matches_manifest};
use super::setup::{WorkspaceInstallSetup, prepare_workspace_install};

pub(super) struct WorkspaceInstallPlan {
    pub(super) setup: WorkspaceInstallSetup,
    pub(super) scenario: InstallScenario,
    pub(super) existing_lockfile: Option<lockfile::Lockfile>,
}

pub(super) fn plan_workspace_install(
    config: &SnpmConfig,
    workspace: &Workspace,
    include_dev: bool,
    frozen_lockfile: bool,
    force: bool,
) -> Result<WorkspaceInstallPlan> {
    let setup = prepare_workspace_install(config, workspace, include_dev, frozen_lockfile)?;
    let (scenario, existing_lockfile) =
        detect_workspace_scenario_early(workspace, &setup.lockfile_path, config, force);
    let (scenario, existing_lockfile) = validate_lockfile_matches_manifest(
        scenario,
        existing_lockfile,
        &setup.root_specs.required,
        &setup.root_specs.optional,
    );

    Ok(WorkspaceInstallPlan {
        setup,
        scenario,
        existing_lockfile,
    })
}

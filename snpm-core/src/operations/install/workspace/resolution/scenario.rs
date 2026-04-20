use crate::lockfile;
use crate::operations::install::utils::FrozenLockfileMode;
use crate::{Result, SnpmConfig, Workspace};

use std::collections::BTreeMap;
use std::path::Path;

use super::super::super::utils::{
    InstallScenario, IntegrityState, build_workspace_integrity_state, check_integrity_path,
    check_store_cache, write_integrity_path,
};

pub(crate) fn detect_workspace_scenario_early(
    workspace: &Workspace,
    lockfile_path: &Path,
    compatible_lockfile: Option<&crate::lockfile::CompatibleLockfile>,
    config: &SnpmConfig,
    frozen_lockfile: FrozenLockfileMode,
    _strict_no_lockfile: bool,
    force: bool,
) -> (InstallScenario, Option<lockfile::Lockfile>) {
    if matches!(
        frozen_lockfile,
        FrozenLockfileMode::No | FrozenLockfileMode::Fix
    ) {
        return (InstallScenario::Cold, None);
    }

    if !lockfile_path.is_file() && compatible_lockfile.is_none() {
        return (InstallScenario::Cold, None);
    }

    let existing = match read_existing_lockfile(lockfile_path, compatible_lockfile, config) {
        Ok(lockfile) => lockfile,
        Err(_) => return (InstallScenario::Cold, None),
    };

    let graph = lockfile::to_graph(&existing);
    let integrity_state = match build_workspace_integrity_state(workspace, &graph) {
        Ok(state) => state,
        Err(_) => return (InstallScenario::Cold, Some(existing)),
    };

    if !force && check_workspace_integrity(&workspace.root, &integrity_state) {
        return (InstallScenario::Hot, Some(existing));
    }

    let cache_check = check_store_cache(config, &graph);
    if cache_check
        .missing
        .iter()
        .any(|package| package.tarball.is_empty())
    {
        return (InstallScenario::Cold, Some(existing));
    }
    if cache_check.missing.is_empty() {
        return (InstallScenario::WarmLinkOnly, Some(existing));
    }

    (InstallScenario::WarmPartialCache, Some(existing))
}

pub(crate) fn validate_lockfile_matches_manifest(
    frozen_lockfile: FrozenLockfileMode,
    lockfile_path: &Path,
    strict_no_lockfile: bool,
    scenario: InstallScenario,
    lockfile: Option<lockfile::Lockfile>,
    required_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
) -> Result<(InstallScenario, Option<lockfile::Lockfile>)> {
    if let Some(ref existing) = lockfile
        && !lockfile::root_specs_match(existing, required_root, optional_root)
    {
        if matches!(frozen_lockfile, FrozenLockfileMode::Frozen) {
            return Err(crate::SnpmError::Lockfile {
                path: lockfile_path.to_path_buf(),
                reason: "manifest dependencies do not match the existing lockfile when using frozen-lockfile"
                    .into(),
            });
        }

        return Ok((InstallScenario::Cold, lockfile));
    }

    if strict_no_lockfile
        && matches!(frozen_lockfile, FrozenLockfileMode::Frozen)
        && lockfile.is_none()
    {
        return Err(crate::SnpmError::Lockfile {
            path: lockfile_path.to_path_buf(),
            reason: "frozen-lockfile requested but lockfile could not be read".into(),
        });
    }

    Ok((scenario, lockfile))
}

pub(crate) fn write_workspace_integrity(
    workspace_root: &Path,
    state: &IntegrityState,
) -> Result<()> {
    write_integrity_path(&workspace_root.join("node_modules"), state)
}

fn check_workspace_integrity(workspace_root: &Path, state: &IntegrityState) -> bool {
    check_integrity_path(&workspace_root.join("node_modules"), state)
}

fn read_existing_lockfile(
    lockfile_path: &Path,
    compatible_lockfile: Option<&crate::lockfile::CompatibleLockfile>,
    config: &SnpmConfig,
) -> crate::Result<crate::lockfile::Lockfile> {
    if lockfile_path.is_file() {
        return lockfile::read(lockfile_path);
    }

    let source = compatible_lockfile.ok_or_else(|| crate::SnpmError::Lockfile {
        path: lockfile_path.to_path_buf(),
        reason: "no lockfile was found".into(),
    })?;

    lockfile::read_compatible_lockfile(source, config)
}

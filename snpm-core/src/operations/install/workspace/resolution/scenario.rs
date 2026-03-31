use crate::lockfile;
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
    config: &SnpmConfig,
    force: bool,
) -> (InstallScenario, Option<lockfile::Lockfile>) {
    if !lockfile_path.is_file() {
        return (InstallScenario::Cold, None);
    }

    let existing = match lockfile::read(lockfile_path) {
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
    if cache_check.missing.is_empty() {
        return (InstallScenario::WarmLinkOnly, Some(existing));
    }

    (InstallScenario::WarmPartialCache, Some(existing))
}

pub(crate) fn validate_lockfile_matches_manifest(
    scenario: InstallScenario,
    lockfile: Option<lockfile::Lockfile>,
    required_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
) -> (InstallScenario, Option<lockfile::Lockfile>) {
    if let Some(ref existing) = lockfile
        && !lockfile::root_specs_match(existing, required_root, optional_root)
    {
        return (InstallScenario::Cold, lockfile);
    }

    (scenario, lockfile)
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

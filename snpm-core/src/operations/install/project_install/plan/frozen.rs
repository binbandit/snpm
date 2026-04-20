use super::types::ProjectInstallPlan;
use crate::console;
use crate::lockfile;
use crate::{Result, SnpmConfig, SnpmError};

use super::super::super::utils::InstallOptions;
use super::super::super::utils::FrozenLockfileMode;

pub(in crate::operations::install::project_install) fn validate_frozen_lockfile(
    config: &SnpmConfig,
    options: &InstallOptions,
    plan: &ProjectInstallPlan,
) -> Result<()> {
    if !matches!(options.frozen_lockfile, FrozenLockfileMode::Frozen) {
        return Ok(());
    }

    let source = if config.frozen_lockfile_default {
        "SNPM_FROZEN_LOCKFILE=1"
    } else {
        "--frozen-lockfile"
    };
    console::verbose(&format!(
        "using frozen lockfile at {} (source: {})",
        plan.lockfile_source_label(),
        source
    ));

    if !plan.additions.is_empty() {
        return Err(SnpmError::Lockfile {
            path: plan.lockfile_path.clone(),
            reason: "cannot add package when using frozen-lockfile".into(),
        });
    }

    let existing = read_frozen_lockfile(plan, config)?;

    if !lockfile::root_specs_match(&existing, &plan.manifest_root, &plan.root_specs.optional) {
        return Err(SnpmError::Lockfile {
            path: plan.lockfile_path.clone(),
            reason: "manifest dependencies do not match the existing lockfile when using frozen-lockfile".into(),
        });
    }

    Ok(())
}

fn read_frozen_lockfile(
    plan: &ProjectInstallPlan,
    config: &SnpmConfig,
) -> Result<crate::lockfile::Lockfile> {
    if plan.lockfile_path.is_file() {
        return lockfile::read(&plan.lockfile_path);
    }

    let source = plan
        .compatible_lockfile
        .as_ref()
        .ok_or_else(|| SnpmError::Lockfile {
            path: plan.lockfile_path.clone(),
            reason: "frozen-lockfile requested but no lockfile was found".into(),
        })?;

    lockfile::read_compatible_lockfile(source, config)
}

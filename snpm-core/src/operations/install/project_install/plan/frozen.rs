use super::types::ProjectInstallPlan;
use crate::console;
use crate::lockfile;
use crate::{Result, SnpmConfig, SnpmError};

use super::super::super::utils::InstallOptions;

pub(in crate::operations::install::project_install) fn validate_frozen_lockfile(
    config: &SnpmConfig,
    options: &InstallOptions,
    plan: &ProjectInstallPlan,
) -> Result<()> {
    if !options.frozen_lockfile && !config.frozen_lockfile_default {
        return Ok(());
    }

    let source = if config.frozen_lockfile_default {
        "SNPM_FROZEN_LOCKFILE=1"
    } else {
        "--frozen-lockfile"
    };
    console::verbose(&format!(
        "using frozen lockfile at {} (source: {})",
        plan.lockfile_path.display(),
        source
    ));

    if !plan.lockfile_path.is_file() {
        return Err(SnpmError::Lockfile {
            path: plan.lockfile_path.clone(),
            reason: "frozen-lockfile requested but snpm-lock.yaml is missing".into(),
        });
    }

    if !plan.additions.is_empty() {
        return Err(SnpmError::Lockfile {
            path: plan.lockfile_path.clone(),
            reason: "cannot add package when using frozen-lockfile".into(),
        });
    }

    let existing = lockfile::read(&plan.lockfile_path)?;
    if !lockfile::root_specs_match(&existing, &plan.manifest_root, &plan.root_specs.optional) {
        return Err(SnpmError::Lockfile {
            path: plan.lockfile_path.clone(),
            reason: "manifest dependencies do not match snpm-lock.yaml when using frozen-lockfile"
                .into(),
        });
    }

    Ok(())
}

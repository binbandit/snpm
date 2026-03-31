mod manifest;
mod staleness;

use crate::{Project, Result, SnpmConfig, Workspace, console};

use staleness::{StalenessReason, check_staleness};

use super::install::InstallOptions;

pub async fn lazy_install(config: &SnpmConfig, project: &mut Project) -> Result<()> {
    let workspace = Workspace::discover(&project.root)?;
    let check = check_staleness(project, workspace.as_ref());

    if !check.is_stale {
        return Ok(());
    }

    console::info(&format!(
        "Installing dependencies ({})...",
        reason_message(check.reason.as_ref())
    ));

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
    let workspace = Workspace::discover(&project.root).unwrap_or_default();
    check_staleness(project, workspace.as_ref()).is_stale
}

fn reason_message(reason: Option<&StalenessReason>) -> &'static str {
    match reason {
        Some(StalenessReason::NoLockfile) => "lockfile missing",
        Some(StalenessReason::NoNodeModules) => "node_modules missing",
        Some(StalenessReason::NoIntegrityFile) => "integrity file missing",
        Some(StalenessReason::IntegrityMismatch) => "integrity mismatch",
        Some(StalenessReason::ManifestChanged) => "manifest changed",
        None => "unknown",
    }
}

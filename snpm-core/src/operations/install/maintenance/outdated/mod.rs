mod config;
mod manifest;
mod results;

use crate::console;
use crate::http;
use crate::resolve;
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace};

use std::time::Instant;

use config::{load_catalog, load_overrides};
use manifest::{build_root_dependencies, build_root_protocols, resolve_manifest_dependencies};
use results::{build_outdated_entries, read_current_versions};

use super::super::utils::OutdatedEntry;

pub async fn outdated(
    config: &SnpmConfig,
    project: &Project,
    include_dev: bool,
    force: bool,
) -> Result<Vec<OutdatedEntry>> {
    let workspace = Workspace::discover(&project.root)?;
    let registry_client = http::create_client()?;
    let overrides = load_overrides(project, workspace.as_ref())?;
    let catalog = load_catalog(project, workspace.as_ref())?;
    let resolved_manifest =
        resolve_manifest_dependencies(project, workspace.as_ref(), catalog.as_ref())?;

    let root_dependencies = build_root_dependencies(
        project,
        workspace.as_ref(),
        &resolved_manifest.dependencies,
        &resolved_manifest.development_dependencies,
        include_dev,
    )?;
    let root_protocols = build_root_protocols(&root_dependencies, &resolved_manifest.protocols);

    console::verbose(&format!(
        "outdated: resolving {} root deps (include_dev={} force={})",
        root_dependencies.len(),
        include_dev,
        force
    ));

    let resolve_started = Instant::now();
    let graph = resolve::resolve(
        config,
        &registry_client,
        &root_dependencies,
        &root_protocols,
        config.min_package_age_days,
        force,
        Some(&overrides),
        None,
        |_package| async { Ok::<(), SnpmError>(()) },
    )
    .await?;

    console::verbose(&format!(
        "outdated: resolve completed in {:.3}s (packages={})",
        resolve_started.elapsed().as_secs_f64(),
        graph.packages.len()
    ));

    let current_versions = read_current_versions(project, workspace.as_ref())?;
    Ok(build_outdated_entries(
        include_dev,
        &resolved_manifest.dependencies,
        &resolved_manifest.development_dependencies,
        &current_versions,
        &graph.root.dependencies,
    ))
}

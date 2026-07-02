mod root;
mod selection;
mod virtual_store;

pub mod bins;
pub mod fs;
pub mod hoist;

use crate::HoistingMode;
use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace};

use std::collections::BTreeMap;
use std::path::PathBuf;

use hoist::{effective_hoisting, hoist_packages};
use root::{link_root_bins, link_root_dependencies, prune_stale_root_entries};
use selection::filter_root_dependencies;
use virtual_store::populate_virtual_store;
pub(crate) use virtual_store::{
    link_virtual_dependencies, local_global_virtual_store_package_ids,
    log_locally_materialized_packages, populate_shared_virtual_store_for_packages,
    resolve_unique_peers,
};

pub fn link(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    project: &Project,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    include_dev: bool,
) -> Result<()> {
    let root_node_modules = project.root.join("node_modules");
    let virtual_store_dir = project.root.join(".snpm");

    std::fs::create_dir_all(&root_node_modules).map_err(|source| SnpmError::WriteFile {
        path: root_node_modules.clone(),
        source,
    })?;
    std::fs::create_dir_all(&virtual_store_dir).map_err(|source| SnpmError::WriteFile {
        path: virtual_store_dir.clone(),
        source,
    })?;

    // `populate_virtual_store` now folds the per-package dep-linking step
    // inside its par_iter (for shared packages this is done inside
    // `populate_shared_virtual_store_for_packages`; for locally-materialized
    // packages it happens in the outer par_iter). The standalone
    // `link_virtual_dependencies` call this used to make was redundant for
    // shared packages (their deps were already wired inside the shared
    // store entry) and the project-view symlinks it created in their
    // node_modules were never consulted by Node since it follows the
    // project-view symlink straight into the shared store.
    let virtual_store_paths = populate_virtual_store(
        &virtual_store_dir,
        graph,
        store_paths,
        config,
        workspace,
        project,
    )?;

    let root_deps_to_link = filter_root_dependencies(project, graph, include_dev);

    // Converge node_modules to the resolved graph: drop root links (and
    // their launchers) for packages that are no longer part of the
    // install set before wiring the current ones. Hoisting runs after
    // and re-creates any hoisted links it still wants.
    let keep: std::collections::BTreeSet<String> = root_deps_to_link
        .iter()
        .map(|(name, _)| (*name).clone())
        .collect();
    let mut virtual_store_roots = vec![virtual_store_dir.clone(), config.virtual_store_dir()];
    if let Some(workspace) = workspace {
        virtual_store_roots.push(workspace.root.join(".snpm"));
    }
    prune_stale_root_entries(&root_node_modules, &keep, &virtual_store_roots);

    link_root_dependencies(&root_deps_to_link, &virtual_store_paths, &root_node_modules)?;
    link_root_bins(&root_deps_to_link, &root_node_modules, graph)?;

    if let HoistingMode::None = effective_hoisting(config, workspace) {
        return Ok(());
    }

    hoist_packages(
        project,
        graph,
        &virtual_store_paths,
        effective_hoisting(config, workspace),
    )
}

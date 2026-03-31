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
use root::{link_root_bins, link_root_dependencies};
use selection::filter_root_dependencies;
use virtual_store::{link_virtual_dependencies, populate_virtual_store};

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

    let virtual_store_paths =
        populate_virtual_store(&virtual_store_dir, graph, store_paths, config)?;

    link_virtual_dependencies(&virtual_store_paths, graph)?;

    let root_deps_to_link = filter_root_dependencies(project, graph, include_dev);

    link_root_dependencies(&root_deps_to_link, &virtual_store_paths, &root_node_modules)?;
    link_root_bins(&root_deps_to_link, &root_node_modules, graph)?;

    if let HoistingMode::None = effective_hoisting(config, workspace) {
        return Ok(());
    }

    hoist_packages(
        config,
        workspace,
        project,
        graph,
        store_paths,
        effective_hoisting(config, workspace),
    )
}

mod external;
mod workspace;

use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Project, Result, Workspace};

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::local::link_local_workspace_deps;
use external::link_external_deps;
use workspace::collect_workspace_protocol_deps;

pub(in crate::operations::install::workspace) fn link_project_dependencies(
    project: &Project,
    workspace: &Workspace,
    graph: &ResolutionGraph,
    virtual_store_paths: &BTreeMap<PackageId, PathBuf>,
    include_dev: bool,
) -> Result<()> {
    let node_modules = project.root.join("node_modules");

    std::fs::create_dir_all(&node_modules).map_err(|source| crate::SnpmError::WriteFile {
        path: node_modules.clone(),
        source,
    })?;

    let (workspace_deps, workspace_dev_deps, workspace_optional_deps) =
        collect_workspace_protocol_deps(project);

    link_external_deps(
        &project.manifest.dependencies,
        &workspace_deps,
        graph,
        virtual_store_paths,
        &node_modules,
    )?;

    if include_dev {
        link_external_deps(
            &project.manifest.dev_dependencies,
            &workspace_dev_deps,
            graph,
            virtual_store_paths,
            &node_modules,
        )?;
    }

    link_external_deps(
        &project.manifest.optional_dependencies,
        &workspace_optional_deps,
        graph,
        virtual_store_paths,
        &node_modules,
    )?;

    link_local_workspace_deps(
        project,
        Some(workspace),
        &workspace_deps,
        &workspace_dev_deps,
        &workspace_optional_deps,
        include_dev,
    )
}

#[cfg(test)]
mod tests;

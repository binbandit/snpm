use crate::workspace::CatalogConfig;
use crate::{Project, Result, Workspace};

use std::collections::BTreeSet;

use super::super::install::{
    RootSpecSet, apply_specs, build_project_root_specs, collect_workspace_root_specs,
};

pub(super) fn build_manifest_root(
    project: &Project,
    workspace: Option<&Workspace>,
) -> Result<RootSpecSet> {
    let catalog = if workspace.is_none() {
        CatalogConfig::load(&project.root)?
    } else {
        None
    };

    let mut local_deps = BTreeSet::new();
    let mut local_dev_deps = BTreeSet::new();
    let mut local_optional_deps = BTreeSet::new();

    let dependencies = apply_specs(
        &project.manifest.dependencies,
        workspace,
        catalog.as_ref(),
        &mut local_deps,
        None,
    )?;

    let development_dependencies = apply_specs(
        &project.manifest.dev_dependencies,
        workspace,
        catalog.as_ref(),
        &mut local_dev_deps,
        None,
    )?;

    let optional_dependencies = apply_specs(
        &project.manifest.optional_dependencies,
        workspace,
        catalog.as_ref(),
        &mut local_optional_deps,
        None,
    )?;

    if let Some(workspace) = workspace {
        collect_workspace_root_specs(workspace, true)
    } else {
        Ok(build_project_root_specs(
            &dependencies,
            &development_dependencies,
            &optional_dependencies,
            true,
        ))
    }
}

use crate::workspace::{CatalogConfig, OverridesConfig};
use crate::{Project, Result, Workspace};

use std::collections::BTreeMap;

pub(super) fn load_overrides(
    project: &Project,
    workspace: Option<&Workspace>,
) -> Result<BTreeMap<String, String>> {
    let root = workspace
        .map(|workspace| &workspace.root)
        .unwrap_or(&project.root);
    let mut overrides = OverridesConfig::load(root)?
        .map(|config| config.overrides)
        .unwrap_or_default();

    // Same effective set the install planners use — otherwise a
    // manifest-declared pin (resolutions / npm overrides / pnpm / snpm)
    // is honored at install time but ignored here, and outdated/upgrade
    // report phantom updates against the unpinned resolution. In a
    // workspace the root manifest owns the overrides, matching install.
    let manifest_owner = workspace
        .and_then(|workspace| {
            workspace
                .projects
                .iter()
                .find(|member| member.root == workspace.root)
        })
        .unwrap_or(project);
    crate::operations::install::overrides::merge_manifest_overrides(
        manifest_owner,
        &mut overrides,
    )?;

    Ok(overrides)
}

pub(super) fn load_catalog(
    project: &Project,
    workspace: Option<&Workspace>,
) -> Result<Option<CatalogConfig>> {
    if workspace.is_some() {
        return Ok(None);
    }

    CatalogConfig::load(&project.root)
}

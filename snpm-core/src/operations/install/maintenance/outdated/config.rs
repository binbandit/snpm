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
    Ok(OverridesConfig::load(root)?
        .map(|config| config.overrides)
        .unwrap_or_default())
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

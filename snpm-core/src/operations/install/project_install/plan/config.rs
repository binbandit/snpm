use crate::workspace::{CatalogConfig, OverridesConfig};
use crate::{Project, Result, Workspace};

use std::collections::BTreeMap;

pub(super) fn load_catalog(
    project: &Project,
    workspace: Option<&Workspace>,
) -> Result<Option<CatalogConfig>> {
    if workspace.is_some() {
        return Ok(None);
    }

    CatalogConfig::load(&project.root)
}

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

    for (name, range) in &project.manifest.resolutions {
        overrides.insert(name.clone(), range.clone());
    }

    if let Some(pnpm) = &project.manifest.pnpm {
        for (name, range) in &pnpm.overrides {
            overrides.insert(name.clone(), range.clone());
        }
    }

    if let Some(snpm) = &project.manifest.snpm {
        for (name, range) in &snpm.overrides {
            overrides.insert(name.clone(), range.clone());
        }
    }

    Ok(overrides)
}

use super::super::types::{CatalogConfig, WorkspaceConfig};
use crate::project::{CatalogMap, NamedCatalogsMap};
use crate::{Result, SnpmError};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub(super) fn empty_workspace_config() -> WorkspaceConfig {
    WorkspaceConfig {
        packages: Vec::new(),
        catalog: BTreeMap::new(),
        catalogs: BTreeMap::new(),
        only_built_dependencies: Vec::new(),
        ignored_built_dependencies: Vec::new(),
        hoisting: None,
    }
}

pub(super) fn read_config(path: &Path) -> Result<WorkspaceConfig> {
    let data = fs::read_to_string(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    serde_yaml::from_str(&data).map_err(|error| SnpmError::WorkspaceConfig {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })
}

pub(super) fn merge_catalog_entries(
    config: &mut WorkspaceConfig,
    catalog: CatalogMap,
    catalogs: NamedCatalogsMap,
) {
    for (name, range) in catalog {
        config.catalog.entry(name).or_insert(range);
    }

    for (catalog_name, entries) in catalogs {
        let target = config.catalogs.entry(catalog_name).or_default();
        for (name, range) in entries {
            target.entry(name).or_insert(range);
        }
    }
}

pub(super) fn merge_snpm_catalog(root: &Path, config: &mut WorkspaceConfig) -> Result<()> {
    if let Some(file) = CatalogConfig::load(root)? {
        merge_catalog_entries(config, file.catalog, file.catalogs);
    }

    Ok(())
}

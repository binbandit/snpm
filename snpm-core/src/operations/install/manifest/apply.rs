use super::protocol::detect_manifest_protocol;
use crate::registry::RegistryProtocol;
use crate::workspace::CatalogConfig;
use crate::{Result, SnpmError, Workspace};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use super::super::workspace::is_local_workspace_dependency;

pub fn apply_specs(
    specs: &BTreeMap<String, String>,
    workspace: Option<&Workspace>,
    catalog: Option<&CatalogConfig>,
    local_set: &mut BTreeSet<String>,
    mut protocol_map: Option<&mut BTreeMap<String, RegistryProtocol>>,
) -> Result<BTreeMap<String, String>> {
    let mut result = BTreeMap::new();

    for (name, value) in specs {
        let resolved = if value.starts_with("catalog:") {
            resolve_catalog_spec(name, value, workspace, catalog)?
        } else {
            value.clone()
        };

        if let Some(workspace) = workspace
            && is_local_workspace_dependency(workspace, name, &resolved)?
        {
            local_set.insert(name.clone());
            continue;
        }

        if let Some(map) = &mut protocol_map
            && let Some(protocol) = detect_manifest_protocol(&resolved)
        {
            map.insert(name.clone(), protocol);
        }

        result.insert(name.clone(), resolved);
    }

    Ok(result)
}

pub fn resolve_catalog_spec(
    name: &str,
    value: &str,
    workspace: Option<&Workspace>,
    catalog: Option<&CatalogConfig>,
) -> Result<String> {
    let selector = &value["catalog:".len()..];
    let source = catalog_source(workspace, catalog)?;

    let range = if selector.is_empty() || selector == "default" {
        source.default_catalog.get(name)
    } else {
        source
            .named_catalogs
            .get(selector)
            .and_then(|entries| entries.get(name))
    };

    match range {
        Some(range) => Ok(range.clone()),
        None => Err(SnpmError::WorkspaceConfig {
            path: source.root_path,
            reason: format!("no catalog entry found for dependency {name} and selector {value}"),
        }),
    }
}

struct CatalogSource<'a> {
    default_catalog: &'a BTreeMap<String, String>,
    named_catalogs: &'a BTreeMap<String, BTreeMap<String, String>>,
    root_path: PathBuf,
}

fn catalog_source<'a>(
    workspace: Option<&'a Workspace>,
    catalog: Option<&'a CatalogConfig>,
) -> Result<CatalogSource<'a>> {
    if let Some(workspace) = workspace {
        return Ok(CatalogSource {
            default_catalog: &workspace.config.catalog,
            named_catalogs: &workspace.config.catalogs,
            root_path: workspace.root.clone(),
        });
    }

    if let Some(catalog) = catalog {
        return Ok(CatalogSource {
            default_catalog: &catalog.catalog,
            named_catalogs: &catalog.catalogs,
            root_path: PathBuf::from("."),
        });
    }

    Err(SnpmError::WorkspaceConfig {
        path: PathBuf::from("."),
        reason: "catalog protocol used but no workspace or catalog configuration found".into(),
    })
}

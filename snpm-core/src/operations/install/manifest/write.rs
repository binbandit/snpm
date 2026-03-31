use crate::resolve::ResolutionGraph;
use crate::workspace::CatalogConfig;
use crate::{Project, Result, Workspace};
use std::collections::BTreeMap;

pub fn write_manifest(
    project: &mut Project,
    graph: &ResolutionGraph,
    additions: &BTreeMap<String, String>,
    dev: bool,
    workspace: Option<&Workspace>,
    catalog: Option<&CatalogConfig>,
) -> Result<()> {
    if additions.is_empty() {
        return Ok(());
    }

    let mut new_dependencies = project.manifest.dependencies.clone();
    let mut new_dev_dependencies = project.manifest.dev_dependencies.clone();

    for (name, dep) in &graph.root.dependencies {
        if !additions.contains_key(name) {
            continue;
        }

        let spec = manifest_spec_for_dependency(name, &dep.resolved.version, workspace, catalog);

        if dev {
            new_dev_dependencies.insert(name.clone(), spec);
        } else {
            new_dependencies.insert(name.clone(), spec);
        }
    }

    project.manifest.dependencies = new_dependencies;
    project.manifest.dev_dependencies = new_dev_dependencies;
    project.write_manifest(&project.manifest)?;

    Ok(())
}

fn manifest_spec_for_dependency(
    name: &str,
    version: &str,
    workspace: Option<&Workspace>,
    catalog: Option<&CatalogConfig>,
) -> String {
    if let Some(selector) = catalog_selector(name, workspace, catalog) {
        return selector;
    }

    format!("^{version}")
}

fn catalog_selector(
    name: &str,
    workspace: Option<&Workspace>,
    catalog: Option<&CatalogConfig>,
) -> Option<String> {
    if let Some(workspace) = workspace {
        return find_catalog_selector(name, &workspace.config.catalog, &workspace.config.catalogs);
    }

    catalog.and_then(|catalog| find_catalog_selector(name, &catalog.catalog, &catalog.catalogs))
}

fn find_catalog_selector(
    name: &str,
    default_catalog: &BTreeMap<String, String>,
    named_catalogs: &BTreeMap<String, BTreeMap<String, String>>,
) -> Option<String> {
    if default_catalog.contains_key(name) {
        return Some("catalog:".to_string());
    }

    for (catalog_name, entries) in named_catalogs {
        if entries.contains_key(name) {
            return Some(format!("catalog:{catalog_name}"));
        }
    }

    None
}

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
    let mut new_optional_dependencies = project.manifest.optional_dependencies.clone();

    for (name, dep) in &graph.root.dependencies {
        let Some(requested) = additions.get(name) else {
            continue;
        };

        let spec = manifest_spec_for_dependency(
            name,
            requested,
            &dep.resolved.version,
            workspace,
            catalog,
        );

        // The package lands in exactly one section; drop it from the
        // others so `snpm add -D x` moves an existing prod entry.
        new_dependencies.remove(name);
        new_dev_dependencies.remove(name);
        new_optional_dependencies.remove(name);

        if dev {
            new_dev_dependencies.insert(name.clone(), spec);
        } else {
            new_dependencies.insert(name.clone(), spec);
        }
    }

    project.manifest.dependencies = new_dependencies;
    project.manifest.dev_dependencies = new_dev_dependencies;
    project.manifest.optional_dependencies = new_optional_dependencies;
    project.write_manifest(&project.manifest)?;

    Ok(())
}

fn manifest_spec_for_dependency(
    name: &str,
    requested: &str,
    version: &str,
    workspace: Option<&Workspace>,
    catalog: Option<&CatalogConfig>,
) -> String {
    if let Some(selector) = catalog_selector(name, workspace, catalog) {
        return selector;
    }

    // Preserve what the user asked for: `snpm add pkg@4.17.20` must pin
    // 4.17.20, `pkg@~1.2.0` must keep the tilde, and git/file/npm-alias
    // specs must survive verbatim. Only a bare `snpm add pkg` (parsed
    // as "latest") defaults to caret-on-resolved-version.
    let requested = requested.trim();
    if requested.is_empty() || requested == "latest" || requested == "*" {
        format!("^{version}")
    } else {
        requested.to_string()
    }
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

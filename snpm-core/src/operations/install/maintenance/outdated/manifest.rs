use crate::registry::RegistryProtocol;
use crate::workspace::CatalogConfig;
use crate::{Project, Result, Workspace};

use std::collections::{BTreeMap, BTreeSet};

use super::super::super::manifest::{apply_specs, build_project_manifest_root};
use super::super::super::workspace::collect_workspace_root_deps;

pub(super) struct ResolvedManifestDependencies {
    pub(super) dependencies: BTreeMap<String, String>,
    pub(super) development_dependencies: BTreeMap<String, String>,
    pub(super) protocols: BTreeMap<String, RegistryProtocol>,
}

pub(super) fn resolve_manifest_dependencies(
    project: &Project,
    workspace: Option<&Workspace>,
    catalog: Option<&CatalogConfig>,
) -> Result<ResolvedManifestDependencies> {
    let mut local_deps = BTreeSet::new();
    let mut local_dev_deps = BTreeSet::new();
    let mut manifest_protocols = BTreeMap::new();

    let dependencies = apply_specs(
        &project.manifest.dependencies,
        workspace,
        catalog,
        &mut local_deps,
        Some(&mut manifest_protocols),
    )?;
    let development_dependencies = apply_specs(
        &project.manifest.dev_dependencies,
        workspace,
        catalog,
        &mut local_dev_deps,
        Some(&mut manifest_protocols),
    )?;

    Ok(ResolvedManifestDependencies {
        dependencies,
        development_dependencies,
        protocols: manifest_protocols,
    })
}

pub(super) fn build_root_dependencies(
    project: &Project,
    workspace: Option<&Workspace>,
    dependencies: &BTreeMap<String, String>,
    development_dependencies: &BTreeMap<String, String>,
    include_dev: bool,
) -> Result<BTreeMap<String, String>> {
    if let Some(workspace) = workspace {
        collect_workspace_root_deps(workspace, include_dev)
    } else {
        Ok(build_project_manifest_root(
            dependencies,
            development_dependencies,
            &project.manifest.optional_dependencies,
            include_dev,
        ))
    }
}

pub(super) fn build_root_protocols(
    root_dependencies: &BTreeMap<String, String>,
    manifest_protocols: &BTreeMap<String, RegistryProtocol>,
) -> BTreeMap<String, RegistryProtocol> {
    let mut root_protocols = BTreeMap::new();

    for name in root_dependencies.keys() {
        let protocol = manifest_protocols
            .get(name)
            .cloned()
            .unwrap_or_else(RegistryProtocol::npm);
        root_protocols.insert(name.clone(), protocol);
    }

    root_protocols
}

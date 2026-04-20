use crate::registry::RegistryProtocol;
use crate::{Result, Workspace};

use std::collections::BTreeMap;

use super::super::super::super::manifest::{RootSpecSet, build_project_root_specs};
use super::ResolvedManifestSpecs;

pub(crate) fn build_root_specs(
    workspace: Option<&Workspace>,
    resolved_manifest: &ResolvedManifestSpecs,
    include_dev: bool,
    overrides: &BTreeMap<String, String>,
) -> Result<RootSpecSet> {
    if let Some(workspace) = workspace {
        super::super::super::super::workspace::collect_workspace_root_specs_with_overrides(
            workspace,
            include_dev,
            overrides,
        )
    } else {
        Ok(build_project_root_specs(
            &resolved_manifest.dependencies,
            &resolved_manifest.development_dependencies,
            &resolved_manifest.optional_dependencies,
            include_dev,
        ))
    }
}

pub(crate) fn merge_root_dependencies(
    manifest_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
    additions: &BTreeMap<String, String>,
) -> BTreeMap<String, String> {
    let mut root_dependencies = manifest_root.clone();

    for (name, range) in optional_root {
        root_dependencies.insert(name.clone(), range.clone());
    }

    for (name, range) in additions {
        root_dependencies.insert(name.clone(), range.clone());
    }

    root_dependencies
}

pub(crate) fn build_root_protocols(
    manifest_root: &BTreeMap<String, String>,
    optional_root: &BTreeMap<String, String>,
    manifest_protocols: &BTreeMap<String, RegistryProtocol>,
    additions: &BTreeMap<String, String>,
    requested_protocols: &BTreeMap<String, RegistryProtocol>,
) -> BTreeMap<String, RegistryProtocol> {
    let mut root_protocols = BTreeMap::new();

    for name in manifest_root.keys().chain(optional_root.keys()) {
        let protocol = manifest_protocols
            .get(name)
            .cloned()
            .unwrap_or_else(RegistryProtocol::npm);
        root_protocols.insert(name.clone(), protocol);
    }

    for name in additions.keys() {
        let protocol = requested_protocols
            .get(name)
            .cloned()
            .unwrap_or_else(RegistryProtocol::npm);
        root_protocols.insert(name.clone(), protocol);
    }

    root_protocols
}

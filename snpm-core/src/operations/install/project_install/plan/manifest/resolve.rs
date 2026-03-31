use crate::{Project, Result, Workspace};

use std::collections::{BTreeMap, BTreeSet};

use super::super::super::super::manifest::apply_specs;
use super::ResolvedManifestSpecs;

pub(crate) fn resolve_manifest_specs(
    project: &Project,
    workspace: Option<&Workspace>,
    catalog: Option<&crate::workspace::CatalogConfig>,
) -> Result<ResolvedManifestSpecs> {
    let mut local_deps = BTreeSet::new();
    let mut local_dev_deps = BTreeSet::new();
    let mut local_optional_deps = BTreeSet::new();
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
    let optional_dependencies = apply_specs(
        &project.manifest.optional_dependencies,
        workspace,
        catalog,
        &mut local_optional_deps,
        Some(&mut manifest_protocols),
    )?;

    Ok(ResolvedManifestSpecs {
        local_deps,
        local_dev_deps,
        local_optional_deps,
        dependencies,
        development_dependencies,
        optional_dependencies,
        protocols: manifest_protocols,
    })
}

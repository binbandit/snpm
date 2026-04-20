use crate::{Project, Result, Workspace};

use std::collections::{BTreeMap, BTreeSet};

use super::super::super::super::manifest::{RootSpecSet, apply_specs, build_project_root_specs};
use super::ranges::insert_workspace_root_dep;

pub fn collect_workspace_root_deps(
    workspace: &Workspace,
    include_dev: bool,
) -> Result<BTreeMap<String, String>> {
    let root_specs = collect_workspace_root_specs(workspace, include_dev)?;
    let mut combined = root_specs.required;

    for (name, range) in root_specs.optional {
        combined.entry(name).or_insert(range);
    }

    Ok(combined)
}

pub fn collect_workspace_root_specs(
    workspace: &Workspace,
    include_dev: bool,
) -> Result<RootSpecSet> {
    collect_workspace_root_specs_with_overrides(workspace, include_dev, &BTreeMap::new())
}

pub(crate) fn collect_workspace_root_specs_with_overrides(
    workspace: &Workspace,
    include_dev: bool,
    overrides: &BTreeMap<String, String>,
) -> Result<RootSpecSet> {
    let mut required = BTreeMap::new();
    let mut optional = BTreeMap::new();

    for member in &workspace.projects {
        let member_specs = build_member_root_specs(workspace, member, include_dev, overrides)?;

        for (name, range) in &member_specs.required {
            insert_workspace_root_dep(&mut required, &workspace.root, &member.root, name, range)?;
        }

        for (name, range) in &member_specs.optional {
            if required.contains_key(name) {
                insert_workspace_root_dep(
                    &mut required,
                    &workspace.root,
                    &member.root,
                    name,
                    range,
                )?;
                continue;
            }

            insert_workspace_root_dep(&mut optional, &workspace.root, &member.root, name, range)?;
        }
    }

    Ok(RootSpecSet { required, optional })
}

fn build_member_root_specs(
    workspace: &Workspace,
    member: &Project,
    include_dev: bool,
    overrides: &BTreeMap<String, String>,
) -> Result<RootSpecSet> {
    let dependencies = apply_member_specs(&member.manifest.dependencies, workspace, overrides)?;
    let optional_dependencies =
        apply_member_specs(&member.manifest.optional_dependencies, workspace, overrides)?;
    let development_dependencies = if include_dev {
        apply_member_specs(&member.manifest.dev_dependencies, workspace, overrides)?
    } else {
        BTreeMap::new()
    };

    Ok(build_project_root_specs(
        &dependencies,
        &development_dependencies,
        &optional_dependencies,
        include_dev,
    ))
}

fn apply_member_specs(
    manifest_deps: &BTreeMap<String, String>,
    workspace: &Workspace,
    overrides: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, String>> {
    let mut local = BTreeSet::new();
    let mut applied = apply_specs(manifest_deps, Some(workspace), None, &mut local, None)?;

    for (name, range) in &mut applied {
        if let Some(override_range) = overrides.get(name) {
            *range = override_range.clone();
        }
    }

    Ok(applied)
}

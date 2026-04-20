use crate::{Project, Result, Workspace};

use super::super::super::utils::InstallOptions;
use super::config::{load_catalog, load_overrides};
use super::manifest::{
    build_root_protocols, build_root_specs, collect_additions, merge_root_dependencies,
    resolve_manifest_specs,
};
use super::types::ProjectInstallPlan;
use crate::lockfile;

pub(in crate::operations::install::project_install) fn prepare_install_plan(
    project: &Project,
    options: &InstallOptions,
) -> Result<ProjectInstallPlan> {
    let (additions, requested_protocols) = collect_additions(project, &options.requested);
    let workspace = Workspace::discover(&project.root)?;
    let catalog = load_catalog(project, workspace.as_ref())?;
    let overrides = load_overrides(project, workspace.as_ref())?;

    let resolved_manifest = resolve_manifest_specs(project, workspace.as_ref(), catalog.as_ref())?;
    let root_specs = build_root_specs(workspace.as_ref(), &resolved_manifest, options.include_dev)?;

    let manifest_root = root_specs.required.clone();
    let optional_root_names = root_specs.optional.keys().cloned().collect();
    let root_dependencies =
        merge_root_dependencies(&manifest_root, &root_specs.optional, &additions);
    let root_protocols = build_root_protocols(
        &manifest_root,
        &root_specs.optional,
        &resolved_manifest.protocols,
        &additions,
        &requested_protocols,
    );

    let lockfile_path = workspace
        .as_ref()
        .map(|workspace| workspace.root.join("snpm-lock.yaml"))
        .unwrap_or_else(|| project.root.join("snpm-lock.yaml"));
    let compatible_lockfile = if workspace.is_none() && !lockfile_path.is_file() {
        lockfile::detect_compatible_lockfile(&project.root)
    } else {
        None
    };
    let is_fresh_install = !lockfile_path.exists() && compatible_lockfile.is_none();

    Ok(ProjectInstallPlan {
        workspace,
        catalog,
        overrides,
        additions,
        local_deps: resolved_manifest.local_deps,
        local_dev_deps: resolved_manifest.local_dev_deps,
        local_optional_deps: resolved_manifest.local_optional_deps,
        manifest_root,
        root_specs,
        root_dependencies,
        root_protocols,
        optional_root_names,
        lockfile_path,
        compatible_lockfile,
        is_fresh_install,
    })
}

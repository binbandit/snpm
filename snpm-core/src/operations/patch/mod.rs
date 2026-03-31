mod inventory;
mod manifest;

use crate::patch::{
    PatchInfo, cleanup_patch_session, create_patch, find_installed_package, list_patches,
    prepare_patch_directory, read_patch_session, remove_patch,
};
use crate::{Project, Result, console};

use std::path::{Path, PathBuf};

use inventory::collect_patches_to_apply;
use manifest::{remove_patch_from_manifest, update_manifest_with_patch};

pub struct PatchStartResult {
    pub package_name: String,
    pub package_version: String,
    pub edit_dir: PathBuf,
}

pub struct PatchCommitResult {
    pub package_name: String,
    pub package_version: String,
    pub patch_path: PathBuf,
}

pub fn start_patch(project: &Project, package_spec: &str) -> Result<PatchStartResult> {
    let (name, version_hint) = parse_package_spec(package_spec);
    let (version, package_path) = find_installed_package(project, &name)?;

    if let Some(hint) = version_hint
        && hint != version
    {
        console::warn(&format!(
            "requested version {} but {} is installed, using installed version",
            hint, version
        ));
    }

    let edit_dir = prepare_patch_directory(&name, &version, &package_path)?;

    Ok(PatchStartResult {
        package_name: name,
        package_version: version,
        edit_dir,
    })
}

pub fn commit_patch(project: &Project, edit_path: &Path) -> Result<PatchCommitResult> {
    let session = read_patch_session(edit_path)?;
    let patch_path = create_patch(project, &session, edit_path)?;

    update_manifest_with_patch(
        project,
        &session.package_name,
        &session.package_version,
        &patch_path,
    )?;

    cleanup_patch_session(edit_path)?;

    Ok(PatchCommitResult {
        package_name: session.package_name,
        package_version: session.package_version,
        patch_path,
    })
}

pub fn remove_package_patch(project: &Project, package_spec: &str) -> Result<Option<PathBuf>> {
    let (name, _) = parse_package_spec(package_spec);
    let removed_path = remove_patch(project, &name)?;

    if removed_path.is_some() {
        remove_patch_from_manifest(project, &name)?;
    }

    Ok(removed_path)
}

pub fn list_project_patches(project: &Project) -> Result<Vec<PatchInfo>> {
    list_patches(project)
}

pub fn get_patches_to_apply(project: &Project) -> Result<Vec<(String, String, PathBuf)>> {
    collect_patches_to_apply(project)
}

fn parse_package_spec(spec: &str) -> (String, Option<String>) {
    match spec.rfind('@') {
        Some(pos) if pos > 0 => (spec[..pos].to_string(), Some(spec[pos + 1..].to_string())),
        _ => (spec.to_string(), None),
    }
}

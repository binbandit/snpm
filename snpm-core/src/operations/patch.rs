use crate::patch::{
    PatchInfo, cleanup_patch_session, create_patch, find_installed_package,
    get_patched_dependencies, list_patches, parse_patch_key, patches_dir, prepare_patch_directory,
    read_patch_session, remove_patch,
};
use crate::project::ManifestSnpm;
use crate::{Project, Result, SnpmError, console};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

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
    let mut result = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for (key, rel_path) in get_patched_dependencies(project) {
        if let Some((name, version)) = parse_patch_key(&key) {
            let patch_path = project.root.join(&rel_path);
            if patch_path.exists() {
                seen.insert(format!("{}@{}", name, version));
                result.push((name, version, patch_path));
            }
        }
    }

    let patches = patches_dir(project);
    if !patches.exists() {
        return Ok(result);
    }

    for entry in fs::read_dir(&patches)
        .map_err(|source| SnpmError::ReadFile {
            path: patches.clone(),
            source,
        })?
        .flatten()
    {
        let path = entry.path();
        let filename = match path.file_name().and_then(|n| n.to_str()) {
            Some(name) if name.ends_with(".patch") => name,
            _ => continue,
        };

        let name_version = match filename.strip_suffix(".patch") {
            Some(nv) => nv,
            None => continue,
        };

        if let Some((name, version)) = parse_patch_key(name_version) {
            let package_name = name.replace('+', "/");
            let key = format!("{}@{}", package_name, version);

            if !seen.contains(&key) {
                result.push((package_name, version, path));
            }
        }
    }

    Ok(result)
}

fn parse_package_spec(spec: &str) -> (String, Option<String>) {
    match spec.rfind('@') {
        Some(pos) if pos > 0 => (spec[..pos].to_string(), Some(spec[pos + 1..].to_string())),
        _ => (spec.to_string(), None),
    }
}

fn update_manifest_with_patch(
    project: &Project,
    package_name: &str,
    package_version: &str,
    patch_path: &Path,
) -> Result<()> {
    let mut manifest = project.manifest.clone();

    let key = format!("{}@{}", package_name, package_version);
    let rel_path = patch_path
        .strip_prefix(&project.root)
        .unwrap_or(patch_path)
        .to_string_lossy()
        .to_string();

    let snpm = manifest.snpm.get_or_insert_with(|| ManifestSnpm {
        overrides: BTreeMap::new(),
        patched_dependencies: None,
    });

    snpm.patched_dependencies
        .get_or_insert_with(BTreeMap::new)
        .insert(key, rel_path);

    project.write_manifest(&manifest)
}

fn remove_patch_from_manifest(project: &Project, package_name: &str) -> Result<()> {
    let mut manifest = project.manifest.clone();
    let mut modified = false;

    modified |= remove_from_patched_deps(&mut manifest.snpm, package_name);
    modified |= remove_from_patched_deps(&mut manifest.pnpm, package_name);

    if modified {
        project.write_manifest(&manifest)?;
    }

    Ok(())
}

fn remove_from_patched_deps<T: HasPatchedDependencies>(
    config: &mut Option<T>,
    package_name: &str,
) -> bool {
    let config = match config.as_mut() {
        Some(c) => c,
        None => return false,
    };

    let patched = match config.patched_dependencies_mut() {
        Some(p) => p,
        None => return false,
    };

    let keys_to_remove: Vec<_> = patched
        .keys()
        .filter(|k| matches_package_name(k, package_name))
        .cloned()
        .collect();

    let removed_any = !keys_to_remove.is_empty();

    for key in keys_to_remove {
        patched.remove(&key);
    }

    if patched.is_empty() {
        config.clear_patched_dependencies();
    }

    removed_any
}

fn matches_package_name(key: &str, package_name: &str) -> bool {
    parse_patch_key(key)
        .map(|(name, _)| name == package_name || name.replace('+', "/") == package_name)
        .unwrap_or(false)
}

trait HasPatchedDependencies {
    fn patched_dependencies_mut(&mut self) -> Option<&mut BTreeMap<String, String>>;
    fn clear_patched_dependencies(&mut self);
}

impl HasPatchedDependencies for crate::project::ManifestSnpm {
    fn patched_dependencies_mut(&mut self) -> Option<&mut BTreeMap<String, String>> {
        self.patched_dependencies.as_mut()
    }
    fn clear_patched_dependencies(&mut self) {
        self.patched_dependencies = None;
    }
}

impl HasPatchedDependencies for crate::project::ManifestPnpm {
    fn patched_dependencies_mut(&mut self) -> Option<&mut BTreeMap<String, String>> {
        self.patched_dependencies.as_mut()
    }
    fn clear_patched_dependencies(&mut self) {
        self.patched_dependencies = None;
    }
}

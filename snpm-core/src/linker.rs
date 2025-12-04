use crate::resolve::{PackageId, ResolutionGraph};
use crate::{HoistingMode, lifecycle};
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

pub fn link(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    project: &Project,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    include_dev: bool,
) -> Result<()> {
    let root_node_modules = project.root.join("node_modules");

    if root_node_modules.exists() {
        fs::remove_dir_all(&root_node_modules).map_err(|source| SnpmError::WriteFile {
            path: root_node_modules.clone(),
            source,
        })?;
    }

    fs::create_dir_all(&root_node_modules).map_err(|source| SnpmError::WriteFile {
        path: root_node_modules.clone(),
        source,
    })?;

    let deps = &project.manifest.dependencies;
    let dev_deps = &project.manifest.dev_dependencies;
    let hoisting = effective_hoisting(config, workspace);

    for (name, dep) in graph.root.dependencies.iter() {
        if !deps.contains_key(name) && !dev_deps.contains_key(name) {
            continue;
        }

        let only_dev = dev_deps.contains_key(name) && !deps.contains_key(name);
        if !include_dev && only_dev {
            continue;
        }

        let id = &dep.resolved;
        let dest = root_node_modules.join(name);
        let mut stack = BTreeSet::new();

        link_package(
            config,
            workspace,
            id,
            &dest,
            &root_node_modules,
            graph,
            store_paths,
            &mut stack,
        )?;
    }

    if !matches!(hoisting, HoistingMode::None) {
        hoist_packages(config, workspace, project, graph, store_paths, hoisting)?;
    }
    Ok(())
}

fn link_package(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    id: &PackageId,
    dest: &Path,
    bin_root: &Path,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    stack: &mut BTreeSet<PackageId>,
) -> Result<()> {
    if stack.contains(id) {
        let store_root = store_paths.get(id).ok_or_else(|| SnpmError::StoreMissing {
            name: id.name.clone(),
            version: id.version.clone(),
        })?;

        if dest.exists() {
            fs::remove_dir_all(dest).map_err(|source| SnpmError::WriteFile {
                path: dest.to_path_buf(),
                source,
            })?;
        }

        let scripts_allowed = lifecycle::is_dep_script_allowed(config, workspace, &id.name);

        if scripts_allowed {
            copy_dir(store_root, dest)?;
        } else {
            link_dir(store_root, dest)?;
        }

        link_bins(dest, bin_root, &id.name)?;
        return Ok(());
    }

    stack.insert(id.clone());

    if dest.exists() {
        fs::remove_dir_all(dest).map_err(|source| SnpmError::WriteFile {
            path: dest.to_path_buf(),
            source,
        })?;
    }

    let store_root = store_paths.get(id).ok_or_else(|| SnpmError::StoreMissing {
        name: id.name.clone(),
        version: id.version.clone(),
    })?;

    let scripts_allowed = lifecycle::is_dep_script_allowed(config, workspace, &id.name);

    if scripts_allowed {
        copy_dir(store_root, dest)?;
    } else {
        link_dir(store_root, dest)?;
    }

    link_bins(dest, bin_root, &id.name)?;

    let package = graph
        .packages
        .get(id)
        .ok_or_else(|| SnpmError::StoreMissing {
            name: id.name.clone(),
            version: id.version.clone(),
        })?;

    for (dep_name, dep_id) in package.dependencies.iter() {
        let node_modules = dest.join("node_modules");
        fs::create_dir_all(&node_modules).map_err(|source| SnpmError::WriteFile {
            path: node_modules.clone(),
            source,
        })?;

        let child_dest = node_modules.join(dep_name);
        link_package(
            config,
            workspace,
            dep_id,
            &child_dest,
            &node_modules,
            graph,
            store_paths,
            stack,
        )?;
    }

    stack.remove(id);

    Ok(())
}

fn hoist_packages(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    project: &Project,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    mode: HoistingMode,
) -> Result<()> {
    if matches!(mode, HoistingMode::None) {
        return Ok(());
    }

    let root_node_modules = project.root.join("node_modules");

    let mut ids_by_name: BTreeMap<&str, Vec<&PackageId>> = BTreeMap::new();

    for id in graph.packages.keys() {
        ids_by_name.entry(&id.name).or_default().push(id);
    }

    for (name, ids) in ids_by_name {
        let should_hoist = match mode {
            HoistingMode::None => false,
            HoistingMode::SingleVersion => ids.len() == 1,
            HoistingMode::All => !ids.is_empty(),
        };

        if !should_hoist {
            continue;
        }

        let id = ids[0];
        let dest = root_node_modules.join(name);

        if dest.exists() {
            continue;
        }

        link_shallow_package(
            config,
            workspace,
            id,
            &dest,
            &root_node_modules,
            store_paths,
        )?;
    }

    Ok(())
}

fn link_shallow_package(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    id: &PackageId,
    dest: &Path,
    bin_root: &Path,
    store_paths: &BTreeMap<PackageId, PathBuf>,
) -> Result<()> {
    if dest.exists() {
        return Ok(());
    }

    let store_root = store_paths.get(id).ok_or_else(|| SnpmError::StoreMissing {
        name: id.name.clone(),
        version: id.version.clone(),
    })?;

    let scripts_allowed = lifecycle::is_dep_script_allowed(config, workspace, &id.name);

    if scripts_allowed {
        copy_dir(store_root, dest)?;
    } else {
        link_dir(store_root, dest)?;
    }

    link_bins(dest, bin_root, &id.name)
}

fn link_bins(dest: &Path, bin_root: &Path, name: &str) -> Result<()> {
    let manifest_path = dest.join("package.json");

    if !manifest_path.is_file() {
        return Ok(());
    }

    let data = fs::read_to_string(&manifest_path).map_err(|source| SnpmError::ReadFile {
        path: manifest_path.clone(),
        source,
    })?;

    let value: Value = serde_json::from_str(&data).map_err(|source| SnpmError::ParseJson {
        path: manifest_path.clone(),
        source,
    })?;

    let bin = value.get("bin");

    if bin.is_none() {
        return Ok(());
    }

    let bin_dir = bin_root.join(".bin");
    fs::create_dir_all(&bin_dir).map_err(|source| SnpmError::WriteFile {
        path: bin_dir.clone(),
        source,
    })?;

    match bin {
        Some(Value::String(script)) => {
            let target = dest.join(script);
            let bin_name = sanitize_bin_name(name);
            create_bin_file(&bin_dir, &bin_name, &target)?;
        }
        Some(Value::Object(map)) => {
            for (entry_name, v) in map.iter() {
                if let Some(script) = v.as_str() {
                    let target = dest.join(script);
                    let bin_name = sanitize_bin_name(entry_name);
                    create_bin_file(&bin_dir, &bin_name, &target)?;
                }
            }
        }
        _ => {}
    }

    Ok(())
}

fn create_bin_file(bin_dir: &Path, name: &str, target: &Path) -> Result<()> {
    if !target.is_file() {
        return Ok(());
    }

    let dest = bin_dir.join(name);

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    if dest.exists() {
        fs::remove_file(&dest).map_err(|source| SnpmError::WriteFile {
            path: dest.clone(),
            source,
        })?;
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;

        if let Err(_source) = symlink(target, &dest) {
            fs::copy(target, &dest).map_err(|source| SnpmError::WriteFile {
                path: dest.clone(),
                source,
            })?;
        }
    }

    #[cfg(windows)]
    {
        use std::os::windows::fs::symlink_file;

        if let Err(_source) = symlink_file(target, &dest) {
            fs::copy(target, &dest).map_err(|source| SnpmError::WriteFile {
                path: dest.clone(),
                source,
            })?;
        }
    }

    Ok(())
}

fn sanitize_bin_name(name: &str) -> String {
    name.rsplit('/').next().unwrap_or(name).to_string()
}

fn link_dir(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest).map_err(|source_err| SnpmError::WriteFile {
        path: dest.to_path_buf(),
        source: source_err,
    })?;

    for entry in fs::read_dir(source).map_err(|source_err| SnpmError::ReadFile {
        path: source.to_path_buf(),
        source: source_err,
    })? {
        let entry = entry.map_err(|source_err| SnpmError::ReadFile {
            path: source.to_path_buf(),
            source: source_err,
        })?;
        let file_type = entry
            .file_type()
            .map_err(|source_err| SnpmError::ReadFile {
                path: entry.path(),
                source: source_err,
            })?;

        let from = entry.path();
        let to = dest.join(entry.file_name());

        if file_type.is_dir() {
            if let Err(_err) = symlink_dir_entry(&from, &to) {
                copy_dir(&from, &to)?;
            }
        } else {
            if let Err(_err) = symlink_file_entry(&from, &to) {
                fs::copy(&from, &to).map_err(|source_err| SnpmError::WriteFile {
                    path: to,
                    source: source_err,
                })?;
            }
        }
    }

    Ok(())
}

fn copy_dir(source: &Path, dest: &Path) -> Result<()> {
    fs::create_dir_all(dest).map_err(|source_err| SnpmError::WriteFile {
        path: dest.to_path_buf(),
        source: source_err,
    })?;

    for entry in fs::read_dir(source).map_err(|source_err| SnpmError::ReadFile {
        path: source.to_path_buf(),
        source: source_err,
    })? {
        let entry = entry.map_err(|source_err| SnpmError::ReadFile {
            path: source.to_path_buf(),
            source: source_err,
        })?;
        let file_type = entry
            .file_type()
            .map_err(|source_err| SnpmError::ReadFile {
                path: entry.path(),
                source: source_err,
            })?;

        let from = entry.path();
        let to = dest.join(entry.file_name());

        if file_type.is_dir() {
            copy_dir(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|source_err| SnpmError::WriteFile {
                path: to,
                source: source_err,
            })?;
        }
    }

    Ok(())
}

#[cfg(unix)]
fn symlink_dir_entry(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::symlink;
    symlink(from, to)
}

#[cfg(windows)]
fn symlink_dir_entry(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::symlink_dir;
    symlink_dir(from, to)
}

#[cfg(unix)]
fn symlink_file_entry(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::symlink;
    symlink(from, to)
}

#[cfg(windows)]
fn symlink_file_entry(from: &Path, to: &Path) -> std::io::Result<()> {
    use std::os::windows::fs::symlink_file;
    symlink_file(from, to)
}

fn effective_hoisting(config: &SnpmConfig, workspace: Option<&Workspace>) -> HoistingMode {
    if let Some(ws) = workspace {
        if let Some(value) = ws.config.hoisting.as_deref() {
            if let Some(mode) = HoistingMode::from_str(value) {
                return mode;
            }
        }
    }

    config.hoisting
}

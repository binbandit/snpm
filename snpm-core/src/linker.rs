use crate::resolve::{PackageId, ResolutionGraph};
use crate::{HoistingMode, LinkBackend, lifecycle};
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

    fs::create_dir_all(&root_node_modules).map_err(|source| SnpmError::WriteFile {
        path: root_node_modules.clone(),
        source,
    })?;

    let deps = &project.manifest.dependencies;
    let dev_deps = &project.manifest.dev_dependencies;
    let hoisting = effective_hoisting(config, workspace);

    let mut linked: BTreeMap<PackageId, PathBuf> = BTreeMap::new();
    let mut root_bin_targets: BTreeMap<String, PathBuf> = BTreeMap::new();

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
            graph,
            store_paths,
            &mut stack,
            &mut linked,
        )?;

        root_bin_targets
            .entry(id.name.clone())
            .or_insert(dest.clone());
    }

    for (pkg_name, pkg_dest) in root_bin_targets.iter() {
        link_bins(pkg_dest, &root_node_modules, pkg_name)?;
    }

    link_bundled_bins_recursive(graph, &linked)?;

    if !matches!(hoisting, HoistingMode::None) {
        hoist_packages(config, workspace, project, graph, store_paths, hoisting)?;
    }

    Ok(())
}

fn link_bundled_bins_recursive(
    graph: &ResolutionGraph,
    linked: &BTreeMap<PackageId, PathBuf>,
) -> Result<()> {
    for (id, dest) in linked.iter() {
        if let Some(package) = graph.packages.get(id) {
            if let Some(ref bundled) = package.bundled_dependencies {
                if !bundled.is_empty() {
                    link_bundled_bins(dest)?;
                }
            }
        }
    }
    Ok(())
}

fn link_bundled_bins(pkg_dest: &Path) -> Result<()> {
    let bundled_modules = pkg_dest.join("node_modules");
    if !bundled_modules.is_dir() {
        return Ok(());
    }

    let bin_dir = bundled_modules.join(".bin");

    let entries = match fs::read_dir(&bundled_modules) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let name = match entry.file_name().to_str() {
            Some(n) => n.to_string(),
            None => continue,
        };

        if name.starts_with('.') {
            continue;
        }

        if name.starts_with('@') {
            if let Ok(scope_entries) = fs::read_dir(&path) {
                for scope_entry in scope_entries.flatten() {
                    let scope_path = scope_entry.path();
                    if scope_path.is_dir() {
                        if let Some(pkg_name) = scope_entry.file_name().to_str() {
                            let full_name = format!("{}/{}", name, pkg_name);
                            link_bins_from_bundled_pkg(&scope_path, &bin_dir, &full_name)?;
                        }
                    }
                }
            }
        } else {
            link_bins_from_bundled_pkg(&path, &bin_dir, &name)?;
        }
    }

    Ok(())
}

fn link_bins_from_bundled_pkg(pkg_path: &Path, bin_dir: &Path, pkg_name: &str) -> Result<()> {
    let manifest_path = pkg_path.join("package.json");
    if !manifest_path.is_file() {
        return Ok(());
    }

    let data = match fs::read_to_string(&manifest_path) {
        Ok(d) => d,
        Err(_) => return Ok(()),
    };

    let value: Value = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(_) => return Ok(()),
    };

    let bin = match value.get("bin") {
        Some(b) => b,
        None => return Ok(()),
    };

    fs::create_dir_all(bin_dir).map_err(|source| SnpmError::WriteFile {
        path: bin_dir.to_path_buf(),
        source,
    })?;

    match bin {
        Value::String(script) => {
            let target = pkg_path.join(script);
            let bin_name = sanitize_bin_name(pkg_name);
            create_bin_file(bin_dir, &bin_name, &target)?;
        }
        Value::Object(map) => {
            for (entry_name, v) in map.iter() {
                if let Some(script) = v.as_str() {
                    let target = pkg_path.join(script);
                    let bin_name = sanitize_bin_name(entry_name);
                    create_bin_file(bin_dir, &bin_name, &target)?;
                }
            }
        }
        _ => {}
    }

    Ok(())
}

fn link_package(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    id: &PackageId,
    dest: &Path,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    stack: &mut BTreeSet<PackageId>,
    linked: &mut BTreeMap<PackageId, PathBuf>,
) -> Result<()> {
    if let Some(existing) = linked.get(id) {
        if dest.exists() {
            fs::remove_dir_all(dest).map_err(|source| SnpmError::WriteFile {
                path: dest.to_path_buf(),
                source,
            })?;
        }

        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        if let Err(_err) = symlink_dir_entry(existing, dest) {
            copy_dir(existing, dest)?;
        }

        return Ok(());
    }

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
            link_dir(config, store_root, dest)?;
        }

        return Ok(());
    }

    stack.insert(id.clone());

    if dest.exists() {
        fs::remove_dir_all(dest).map_err(|source| SnpmError::WriteFile {
            path: dest.to_path_buf(),
            source,
        })?;
    }

    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    let store_root = store_paths.get(id).ok_or_else(|| SnpmError::StoreMissing {
        name: id.name.clone(),
        version: id.version.clone(),
    })?;

    let package = graph
        .packages
        .get(id)
        .ok_or_else(|| SnpmError::GraphMissing {
            name: id.name.clone(),
            version: id.version.clone(),
        })?;

    let scripts_allowed = lifecycle::is_dep_script_allowed(config, workspace, &id.name);
    let has_nested_deps = !package.dependencies.is_empty();

    if scripts_allowed || has_nested_deps {
        if scripts_allowed {
            copy_dir(store_root, dest)?;
        } else {
            link_dir(config, store_root, dest)?;
        }
    } else {
        if symlink_dir_entry(store_root, dest).is_err() {
            link_dir(config, store_root, dest)?;
        }
    }

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
            graph,
            store_paths,
            stack,
            linked,
        )?;
    }

    stack.remove(id);
    linked.insert(id.clone(), dest.to_path_buf());

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

        link_shallow_package(config, workspace, id, &dest, store_paths)?;
    }

    Ok(())
}

fn link_shallow_package(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    id: &PackageId,
    dest: &Path,
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
        link_dir(config, store_root, dest)?;
    }

    Ok(())
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

fn link_dir(config: &SnpmConfig, source: &Path, dest: &Path) -> Result<()> {
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
            let is_node_modules = entry.file_name() == "node_modules";

            if is_node_modules {
                link_dir(config, &from, &to)?;
            } else {
                if let Err(_err) = symlink_dir_entry(&from, &to) {
                    copy_dir(&from, &to)?;
                }
            }
        } else {
            link_file(config, &from, &to)?;
        }
    }

    Ok(())
}

fn link_file(config: &SnpmConfig, from: &Path, to: &Path) -> Result<()> {
    match config.link_backend {
        LinkBackend::Auto => {
            if fs::hard_link(from, to).is_ok() {
                return Ok(());
            }

            if symlink_file_entry(from, to).is_ok() {
                return Ok(());
            }

            fs::copy(from, to).map_err(|source_err| SnpmError::WriteFile {
                path: to.to_path_buf(),
                source: source_err,
            })?;
        }
        LinkBackend::Hardlink => {
            if fs::hard_link(from, to).is_err() {
                fs::copy(from, to).map_err(|source_err| SnpmError::WriteFile {
                    path: to.to_path_buf(),
                    source: source_err,
                })?;
            }
        }
        LinkBackend::Symlink => {
            if symlink_file_entry(from, to).is_err() {
                fs::copy(from, to).map_err(|source_err| SnpmError::WriteFile {
                    path: to.to_path_buf(),
                    source: source_err,
                })?;
            }
        }
        LinkBackend::Copy => {
            fs::copy(from, to).map_err(|source_err| SnpmError::WriteFile {
                path: to.to_path_buf(),
                source: source_err,
            })?;
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

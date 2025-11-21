use serde_json::Value;

use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Project, Result, SnpmError};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn link(
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

    for (name, dep) in graph.root.dependencies.iter() {
        let only_dev = dev_deps.contains_key(name) && !deps.contains_key(name);
        if !include_dev && only_dev {
            continue;
        }

        let id = &dep.resolved;
        let dest = root_node_modules.join(name);
        link_package(id, &dest, &root_node_modules, graph, store_paths)?;
    }

    Ok(())
}

fn link_package(
    id: &PackageId,
    dest: &Path,
    bin_root: &Path,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
) -> Result<()> {
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

    copy_dir(store_root, dest)?;
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
        link_package(dep_id, &child_dest, &node_modules, graph, store_paths)?;
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

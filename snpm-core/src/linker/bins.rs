use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Result, SnpmError};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub fn link_bundled_bins_recursive(
    graph: &ResolutionGraph,
    linked: &BTreeMap<PackageId, PathBuf>,
) -> Result<()> {
    for (id, dest) in linked.iter() {
        if let Some(package) = graph.packages.get(id)
            && let Some(ref bundled) = package.bundled_dependencies
            && !bundled.is_empty()
        {
            link_bundled_bins(dest)?;
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
                    if scope_path.is_dir()
                        && let Some(pkg_name) = scope_entry.file_name().to_str()
                    {
                        let full_name = format!("{}/{}", name, pkg_name);
                        link_bins_from_bundled_pkg(&scope_path, &bin_dir, &full_name)?;
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

pub fn link_bins(dest: &Path, bin_root: &Path, name: &str) -> Result<()> {
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

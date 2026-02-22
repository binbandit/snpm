use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Result, SnpmError};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

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
        let Ok(file_type) = entry.file_type() else {
            continue;
        };

        if !file_type.is_dir() || file_type.is_symlink() {
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
                    let Ok(scope_type) = scope_entry.file_type() else {
                        continue;
                    };

                    if scope_type.is_dir()
                        && !scope_type.is_symlink()
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
            let Some(target) = resolve_bin_target(pkg_path, script) else {
                return Ok(());
            };
            let Some(bin_name) = sanitize_bin_name(pkg_name) else {
                return Ok(());
            };
            create_bin_file(bin_dir, &bin_name, &target)?;
        }
        Value::Object(map) => {
            for (entry_name, v) in map.iter() {
                if let Some(script) = v.as_str() {
                    let Some(target) = resolve_bin_target(pkg_path, script) else {
                        continue;
                    };
                    let Some(bin_name) = sanitize_explicit_bin_name(entry_name) else {
                        continue;
                    };
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
            let Some(target) = resolve_bin_target(dest, script) else {
                return Ok(());
            };
            let Some(bin_name) = sanitize_bin_name(name) else {
                return Ok(());
            };
            create_bin_file(&bin_dir, &bin_name, &target)?;
        }
        Some(Value::Object(map)) => {
            for (entry_name, v) in map.iter() {
                if let Some(script) = v.as_str() {
                    let Some(target) = resolve_bin_target(dest, script) else {
                        continue;
                    };
                    let Some(bin_name) = sanitize_explicit_bin_name(entry_name) else {
                        continue;
                    };
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

fn sanitize_bin_name(name: &str) -> Option<String> {
    let candidate = name.rsplit('/').next().unwrap_or(name);

    if candidate.is_empty()
        || candidate == "."
        || candidate == ".."
        || candidate.contains('/')
        || candidate.contains('\\')
        || candidate.contains(':')
        || candidate.contains('\0')
    {
        return None;
    }

    Some(candidate.to_string())
}

fn sanitize_explicit_bin_name(name: &str) -> Option<String> {
    if name.is_empty()
        || name == "."
        || name == ".."
        || name.contains('/')
        || name.contains('\\')
        || name.contains(':')
        || name.contains('\0')
    {
        return None;
    }

    Some(name.to_string())
}

fn resolve_bin_target(root: &Path, script: &str) -> Option<PathBuf> {
    let script_path = Path::new(script);
    if script_path.is_absolute() {
        return None;
    }

    let mut target = root.to_path_buf();
    for component in script_path.components() {
        match component {
            Component::Normal(part) => target.push(part),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => return None,
        }
    }

    Some(target)
}

#[cfg(test)]
mod tests {
    use super::link_bins;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn blocks_traversal_in_bin_name_and_script() {
        let tmp = tempdir().unwrap();
        let root = tmp.path();
        let pkg_dir = root.join("node_modules").join("pkg");
        fs::create_dir_all(&pkg_dir).unwrap();

        fs::write(pkg_dir.join("safe.js"), "#!/usr/bin/env node\n").unwrap();

        let manifest = r#"{
            "name": "pkg",
            "version": "1.0.0",
            "bin": {
                "ok": "safe.js",
                "../escape": "safe.js",
                "escape-script": "../outside.js"
            }
        }"#;
        fs::write(pkg_dir.join("package.json"), manifest).unwrap();

        link_bins(&pkg_dir, &root.join("node_modules"), "pkg").unwrap();

        let bin_dir = root.join("node_modules").join(".bin");
        assert!(bin_dir.join("ok").exists());
        assert!(!bin_dir.join("escape").exists());
        assert!(!bin_dir.join("escape-script").exists());
        assert!(!root.join("node_modules").join("outside.js").exists());
    }
}

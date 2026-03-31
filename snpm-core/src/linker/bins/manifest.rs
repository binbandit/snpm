use super::names::{resolve_bin_target, sanitize_bin_name, sanitize_explicit_bin_name};
use super::writer::create_bin_file;
use crate::{Result, SnpmError};

use serde_json::Value;
use std::fs;
use std::path::Path;

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
        path: manifest_path,
        source,
    })?;

    let Some(bin) = value.get("bin") else {
        return Ok(());
    };

    let bin_dir = bin_root.join(".bin");
    fs::create_dir_all(&bin_dir).map_err(|source| SnpmError::WriteFile {
        path: bin_dir.clone(),
        source,
    })?;

    link_bin_entries(dest, &bin_dir, name, bin)
}

pub(in crate::linker::bins) fn link_bins_from_bundled_pkg(
    pkg_path: &Path,
    bin_dir: &Path,
    pkg_name: &str,
) -> Result<()> {
    let manifest_path = pkg_path.join("package.json");
    if !manifest_path.is_file() {
        return Ok(());
    }

    let Ok(data) = fs::read_to_string(&manifest_path) else {
        return Ok(());
    };
    let Ok(value) = serde_json::from_str::<Value>(&data) else {
        return Ok(());
    };
    let Some(bin) = value.get("bin") else {
        return Ok(());
    };

    fs::create_dir_all(bin_dir).map_err(|source| SnpmError::WriteFile {
        path: bin_dir.to_path_buf(),
        source,
    })?;

    link_bin_entries(pkg_path, bin_dir, pkg_name, bin)
}

fn link_bin_entries(pkg_path: &Path, bin_dir: &Path, pkg_name: &str, bin: &Value) -> Result<()> {
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
            for (entry_name, value) in map {
                let Some(script) = value.as_str() else {
                    continue;
                };
                let Some(target) = resolve_bin_target(pkg_path, script) else {
                    continue;
                };
                let Some(bin_name) = sanitize_explicit_bin_name(entry_name) else {
                    continue;
                };
                create_bin_file(bin_dir, &bin_name, &target)?;
            }
        }
        _ => {}
    }

    Ok(())
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

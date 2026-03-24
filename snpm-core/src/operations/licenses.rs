use crate::{Result, SnpmError};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct LicenseEntry {
    pub name: String,
    pub version: String,
    pub license: String,
}

pub fn collect_licenses(node_modules: &Path) -> Result<Vec<LicenseEntry>> {
    let virtual_store = node_modules.join(".snpm");

    if !virtual_store.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let mut seen = BTreeMap::new();

    let store_entries = fs::read_dir(&virtual_store).map_err(|source| SnpmError::ReadFile {
        path: virtual_store.clone(),
        source,
    })?;

    for entry in store_entries.flatten() {
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }

        let nm_dir = entry_path.join("node_modules");
        if !nm_dir.is_dir() {
            continue;
        }

        for pkg_entry in fs::read_dir(&nm_dir).into_iter().flatten().flatten() {
            let pkg_path = pkg_entry.path();
            if !pkg_path.is_dir() {
                // Could be a scoped package — check one level deeper
                continue;
            }

            // Handle scoped packages (@scope/name)
            let name_str = pkg_entry.file_name().to_string_lossy().to_string();
            if name_str.starts_with('@') {
                for scoped_entry in fs::read_dir(&pkg_path).into_iter().flatten().flatten() {
                    let scoped_path = scoped_entry.path();
                    let full_name = format!("{}/{}", name_str, scoped_entry.file_name().to_string_lossy());
                    if let Some(entry) = read_license_from_dir(&scoped_path, &full_name) {
                        let key = format!("{}@{}", entry.name, entry.version);
                        if let std::collections::btree_map::Entry::Vacant(e) = seen.entry(key) {
                            e.insert(true);
                            entries.push(entry);
                        }
                    }
                }
            } else if let Some(entry) = read_license_from_dir(&pkg_path, &name_str) {
                let key = format!("{}@{}", entry.name, entry.version);
                if let std::collections::btree_map::Entry::Vacant(e) = seen.entry(key) {
                    e.insert(true);
                    entries.push(entry);
                }
            }
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn read_license_from_dir(dir: &Path, fallback_name: &str) -> Option<LicenseEntry> {
    let pkg_json = dir.join("package.json");
    let data = fs::read_to_string(&pkg_json).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&data).ok()?;

    let name = manifest
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or(fallback_name)
        .to_string();

    let version = manifest
        .get("version")
        .and_then(|v| v.as_str())
        .unwrap_or("0.0.0")
        .to_string();

    let license = extract_license(&manifest);

    Some(LicenseEntry {
        name,
        version,
        license,
    })
}

fn extract_license(manifest: &serde_json::Value) -> String {
    // Try "license" field (string)
    if let Some(license) = manifest.get("license").and_then(|l| l.as_str()) {
        return license.to_string();
    }

    // Try "license" as object with "type" field
    if let Some(obj) = manifest.get("license").and_then(|l| l.as_object())
        && let Some(t) = obj.get("type").and_then(|t| t.as_str())
    {
        return t.to_string();
    }

    // Try "licenses" array
    if let Some(arr) = manifest.get("licenses").and_then(|l| l.as_array()) {
        let types: Vec<&str> = arr
            .iter()
            .filter_map(|l| l.get("type").and_then(|t| t.as_str()))
            .collect();
        if !types.is_empty() {
            return types.join(" OR ");
        }
    }

    "UNKNOWN".to_string()
}

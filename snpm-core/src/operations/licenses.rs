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
    let mut seen: BTreeMap<String, bool> = BTreeMap::new();

    let store_entries = fs::read_dir(&virtual_store).map_err(|source| SnpmError::ReadFile {
        path: virtual_store.clone(),
        source,
    })?;

    for entry in store_entries.flatten() {
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }

        let node_modules_dir = entry_path.join("node_modules");
        if !node_modules_dir.is_dir() {
            continue;
        }

        for package_entry in fs::read_dir(&node_modules_dir)
            .into_iter()
            .flatten()
            .flatten()
        {
            let package_path = package_entry.path();
            if !package_path.is_dir() {
                continue;
            }

            let dir_name = package_entry.file_name().to_string_lossy().to_string();
            if dir_name.starts_with('@') {
                for scoped_entry in fs::read_dir(&package_path).into_iter().flatten().flatten() {
                    let scoped_path = scoped_entry.path();
                    let full_name = format!(
                        "{}/{}",
                        dir_name,
                        scoped_entry.file_name().to_string_lossy()
                    );
                    if let Some(license_entry) =
                        read_license_from_directory(&scoped_path, &full_name)
                    {
                        let key = format!("{}@{}", license_entry.name, license_entry.version);
                        if let std::collections::btree_map::Entry::Vacant(vacant) = seen.entry(key)
                        {
                            vacant.insert(true);
                            entries.push(license_entry);
                        }
                    }
                }
            } else if let Some(license_entry) =
                read_license_from_directory(&package_path, &dir_name)
            {
                let key = format!("{}@{}", license_entry.name, license_entry.version);
                if let std::collections::btree_map::Entry::Vacant(vacant) = seen.entry(key) {
                    vacant.insert(true);
                    entries.push(license_entry);
                }
            }
        }
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn read_license_from_directory(directory: &Path, fallback_name: &str) -> Option<LicenseEntry> {
    let manifest_path = directory.join("package.json");
    let content = fs::read_to_string(&manifest_path).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&content).ok()?;

    let name = manifest
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or(fallback_name)
        .to_string();

    let version = manifest
        .get("version")
        .and_then(|value| value.as_str())
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
    if let Some(license) = manifest.get("license").and_then(|value| value.as_str()) {
        return license.to_string();
    }

    if let Some(object) = manifest.get("license").and_then(|value| value.as_object())
        && let Some(license_type) = object.get("type").and_then(|value| value.as_str())
    {
        return license_type.to_string();
    }

    // Legacy "licenses" array format used by some older packages
    if let Some(array) = manifest.get("licenses").and_then(|value| value.as_array()) {
        let types: Vec<&str> = array
            .iter()
            .filter_map(|entry| entry.get("type").and_then(|value| value.as_str()))
            .collect();
        if !types.is_empty() {
            return types.join(" OR ");
        }
    }

    "UNKNOWN".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_license_string() {
        let manifest = serde_json::json!({ "license": "MIT" });
        assert_eq!(extract_license(&manifest), "MIT");
    }

    #[test]
    fn extract_license_object_with_type() {
        let manifest = serde_json::json!({ "license": { "type": "ISC", "url": "https://example.com" } });
        assert_eq!(extract_license(&manifest), "ISC");
    }

    #[test]
    fn extract_license_legacy_array() {
        let manifest = serde_json::json!({
            "licenses": [
                { "type": "MIT" },
                { "type": "Apache-2.0" }
            ]
        });
        assert_eq!(extract_license(&manifest), "MIT OR Apache-2.0");
    }

    #[test]
    fn extract_license_unknown() {
        let manifest = serde_json::json!({ "name": "pkg" });
        assert_eq!(extract_license(&manifest), "UNKNOWN");
    }

    #[test]
    fn extract_license_empty_licenses_array() {
        let manifest = serde_json::json!({ "licenses": [] });
        assert_eq!(extract_license(&manifest), "UNKNOWN");
    }

    #[test]
    fn read_license_from_directory_works() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_dir = dir.path().join("my-pkg");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(
            pkg_dir.join("package.json"),
            r#"{ "name": "my-pkg", "version": "1.0.0", "license": "MIT" }"#,
        ).unwrap();

        let entry = read_license_from_directory(&pkg_dir, "fallback").unwrap();
        assert_eq!(entry.name, "my-pkg");
        assert_eq!(entry.version, "1.0.0");
        assert_eq!(entry.license, "MIT");
    }

    #[test]
    fn read_license_from_directory_uses_fallback_name() {
        let dir = tempfile::tempdir().unwrap();
        let pkg_dir = dir.path().join("unnamed");
        std::fs::create_dir_all(&pkg_dir).unwrap();
        std::fs::write(
            pkg_dir.join("package.json"),
            r#"{ "version": "2.0.0", "license": "ISC" }"#,
        ).unwrap();

        let entry = read_license_from_directory(&pkg_dir, "fallback-name").unwrap();
        assert_eq!(entry.name, "fallback-name");
    }

    #[test]
    fn read_license_from_directory_returns_none_when_no_manifest() {
        let dir = tempfile::tempdir().unwrap();
        assert!(read_license_from_directory(dir.path(), "test").is_none());
    }
}

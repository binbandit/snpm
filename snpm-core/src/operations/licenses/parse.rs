use std::fs;
use std::path::Path;

#[derive(Debug, Clone, serde::Serialize)]
pub struct LicenseEntry {
    pub name: String,
    pub version: String,
    pub license: String,
}

pub(super) fn read_license_from_directory(
    directory: &Path,
    fallback_name: &str,
) -> Option<LicenseEntry> {
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

pub(super) fn extract_license(manifest: &serde_json::Value) -> String {
    if let Some(license) = manifest.get("license").and_then(|value| value.as_str()) {
        return license.to_string();
    }

    if let Some(object) = manifest.get("license").and_then(|value| value.as_object())
        && let Some(license_type) = object.get("type").and_then(|value| value.as_str())
    {
        return license_type.to_string();
    }

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

use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug)]
pub struct PackageManagerReference {
    pub name: String,
    pub version: String,
}

#[derive(Deserialize)]
struct PartialManifest {
    #[serde(rename = "packageManager")]
    package_manager: Option<String>,
}

pub fn find_package_manager() -> anyhow::Result<Option<PackageManagerReference>> {
    let current_dir = std::env::current_dir()?;
    find_package_manager_from(&current_dir)
}

pub fn find_package_manager_from(start: &Path) -> anyhow::Result<Option<PackageManagerReference>> {
    let mut current = Some(start);

    while let Some(dir) = current {
        let manifest_path = dir.join("package.json");

        if manifest_path.is_file() {
            if let Some(reference) = parse_package_manager(&manifest_path)? {
                return Ok(Some(reference));
            }
        }

        current = dir.parent();
    }

    Ok(None)
}

fn parse_package_manager(path: &Path) -> anyhow::Result<Option<PackageManagerReference>> {
    let content = fs::read_to_string(path)?;
    let manifest: PartialManifest = serde_json::from_str(&content)?;

    let Some(field) = manifest.package_manager else {
        return Ok(None);
    };

    parse_package_manager_field(&field)
}

fn parse_package_manager_field(field: &str) -> anyhow::Result<Option<PackageManagerReference>> {
    let at_index = match field.rfind('@') {
        Some(idx) if idx > 0 => idx,
        _ => return Ok(None),
    };

    let name = field[..at_index].to_string();
    let version_part = &field[at_index + 1..];

    let version = if let Some(plus_index) = version_part.find('+') {
        version_part[..plus_index].to_string()
    } else {
        version_part.to_string()
    };

    Ok(Some(PackageManagerReference { name, version }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_package_manager_field() {
        let result = parse_package_manager_field("snpm@1.0.0").unwrap().unwrap();
        assert_eq!(result.name, "snpm");
        assert_eq!(result.version, "1.0.0");

        let result = parse_package_manager_field("snpm@1.0.0+sha256.abc123")
            .unwrap()
            .unwrap();
        assert_eq!(result.name, "snpm");
        assert_eq!(result.version, "1.0.0");

        let result = parse_package_manager_field("@scoped/pkg@2.0.0")
            .unwrap()
            .unwrap();
        assert_eq!(result.name, "@scoped/pkg");
        assert_eq!(result.version, "2.0.0");
    }
}

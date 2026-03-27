use crate::{Result, SnpmError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

pub type CatalogMap = BTreeMap<String, String>;
pub type NamedCatalogsMap = BTreeMap<String, BTreeMap<String, String>>;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum WorkspacesField {
    Patterns(Vec<String>),
    Object {
        packages: Vec<String>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        catalog: CatalogMap,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        catalogs: NamedCatalogsMap,
    },
}

impl WorkspacesField {
    pub fn patterns(&self) -> &[String] {
        match self {
            WorkspacesField::Patterns(p) => p,
            WorkspacesField::Object { packages, .. } => packages,
        }
    }

    pub fn into_parts(self) -> (Vec<String>, CatalogMap, NamedCatalogsMap) {
        match self {
            WorkspacesField::Patterns(p) => (p, BTreeMap::new(), BTreeMap::new()),
            WorkspacesField::Object {
                packages,
                catalog,
                catalogs,
            } => (packages, catalog, catalogs),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub name: Option<String>,
    pub version: Option<String>,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    #[serde(default)]
    pub dev_dependencies: BTreeMap<String, String>,
    #[serde(default)]
    pub optional_dependencies: BTreeMap<String, String>,
    #[serde(default)]
    pub scripts: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<BinField>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main: Option<String>,
    #[serde(default)]
    pub pnpm: Option<ManifestPnpm>,
    #[serde(default)]
    pub snpm: Option<ManifestSnpm>,
    #[serde(default)]
    pub workspaces: Option<WorkspacesField>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BinField {
    Single(String),
    Map(BTreeMap<String, String>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestPnpm {
    #[serde(default)]
    pub overrides: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patched_dependencies: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSnpm {
    #[serde(default)]
    pub overrides: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patched_dependencies: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone)]
pub struct Project {
    pub root: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: Manifest,
}

impl Project {
    pub fn discover(start: &Path) -> Result<Self> {
        let mut current = Some(start);

        while let Some(dir) = current {
            let candidate = dir.join("package.json");
            if candidate.is_file() {
                return Self::from_manifest_path(candidate);
            }
            current = dir.parent();
        }

        Err(SnpmError::ManifestMissing {
            path: start.to_path_buf(),
        })
    }

    pub fn from_manifest_path(path: PathBuf) -> Result<Self> {
        let data = fs::read_to_string(&path).map_err(|source| SnpmError::ReadFile {
            path: path.clone(),
            source,
        })?;

        let manifest: Manifest =
            serde_json::from_str(&data).map_err(|source| SnpmError::ParseJson {
                path: path.clone(),
                source,
            })?;

        let root =
            path.parent()
                .map(Path::to_path_buf)
                .ok_or_else(|| SnpmError::ManifestInvalid {
                    path: path.clone(),
                    reason: "manifest has no parent directory".into(),
                })?;

        Ok(Project {
            root,
            manifest_path: path,
            manifest,
        })
    }

    pub fn write_manifest(&self, manifest: &Manifest) -> Result<()> {
        let mut data =
            serde_json::to_string_pretty(manifest).map_err(|e| SnpmError::SerializeJson {
                path: self.manifest_path.clone(),
                reason: e.to_string(),
            })?;
        data.push('\n');

        fs::write(&self.manifest_path, data).map_err(|source| SnpmError::WriteFile {
            path: self.manifest_path.clone(),
            source,
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn manifest_deserializes_minimal() {
        let json = r#"{ "name": "my-pkg", "version": "1.0.0" }"#;
        let manifest: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.name.as_deref(), Some("my-pkg"));
        assert_eq!(manifest.version.as_deref(), Some("1.0.0"));
        assert!(manifest.dependencies.is_empty());
        assert!(manifest.dev_dependencies.is_empty());
    }

    #[test]
    fn manifest_deserializes_with_deps() {
        let json = r#"{
            "name": "test",
            "dependencies": { "lodash": "^4.0.0" },
            "devDependencies": { "jest": "^29.0.0" }
        }"#;
        let manifest: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(
            manifest.dependencies.get("lodash").map(String::as_str),
            Some("^4.0.0")
        );
        assert_eq!(
            manifest.dev_dependencies.get("jest").map(String::as_str),
            Some("^29.0.0")
        );
    }

    #[test]
    fn manifest_deserializes_scripts() {
        let json = r#"{ "scripts": { "build": "tsc", "test": "jest" } }"#;
        let manifest: Manifest = serde_json::from_str(json).unwrap();
        assert_eq!(
            manifest.scripts.get("build").map(String::as_str),
            Some("tsc")
        );
    }

    #[test]
    fn manifest_deserializes_bin_single() {
        let json = r#"{ "bin": "./cli.js" }"#;
        let manifest: Manifest = serde_json::from_str(json).unwrap();
        assert!(matches!(manifest.bin, Some(BinField::Single(_))));
    }

    #[test]
    fn manifest_deserializes_bin_map() {
        let json = r#"{ "bin": { "cmd1": "./a.js", "cmd2": "./b.js" } }"#;
        let manifest: Manifest = serde_json::from_str(json).unwrap();
        assert!(matches!(manifest.bin, Some(BinField::Map(_))));
    }

    #[test]
    fn workspaces_field_patterns_array() {
        let json = r#"{ "workspaces": ["packages/*"] }"#;
        let manifest: Manifest = serde_json::from_str(json).unwrap();
        let ws = manifest.workspaces.unwrap();
        assert_eq!(ws.patterns(), &["packages/*".to_string()]);
    }

    #[test]
    fn workspaces_field_object() {
        let json = r#"{ "workspaces": { "packages": ["apps/*"], "catalog": { "react": "^18.0.0" } } }"#;
        let manifest: Manifest = serde_json::from_str(json).unwrap();
        let ws = manifest.workspaces.unwrap();
        assert_eq!(ws.patterns(), &["apps/*".to_string()]);
        let (patterns, catalog, _catalogs) = ws.into_parts();
        assert_eq!(patterns, vec!["apps/*".to_string()]);
        assert_eq!(catalog.get("react").map(String::as_str), Some("^18.0.0"));
    }

    #[test]
    fn workspaces_into_parts_patterns() {
        let ws = WorkspacesField::Patterns(vec!["a/*".to_string(), "b/*".to_string()]);
        let (patterns, catalog, catalogs) = ws.into_parts();
        assert_eq!(patterns, vec!["a/*", "b/*"]);
        assert!(catalog.is_empty());
        assert!(catalogs.is_empty());
    }

    #[test]
    fn project_discover_finds_manifest() {
        let dir = tempdir().unwrap();
        let sub = dir.path().join("packages/foo");
        std::fs::create_dir_all(&sub).unwrap();
        std::fs::write(
            dir.path().join("package.json"),
            r#"{ "name": "root" }"#,
        )
        .unwrap();

        let project = Project::discover(&sub).unwrap();
        assert_eq!(project.manifest.name.as_deref(), Some("root"));
    }

    #[test]
    fn project_discover_fails_without_manifest() {
        let dir = tempdir().unwrap();
        let result = Project::discover(dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn project_from_manifest_path() {
        let dir = tempdir().unwrap();
        let manifest_path = dir.path().join("package.json");
        std::fs::write(
            &manifest_path,
            r#"{ "name": "test-pkg", "version": "2.0.0" }"#,
        )
        .unwrap();

        let project = Project::from_manifest_path(manifest_path).unwrap();
        assert_eq!(project.manifest.name.as_deref(), Some("test-pkg"));
        assert_eq!(project.manifest.version.as_deref(), Some("2.0.0"));
        assert_eq!(project.root, dir.path());
    }

    #[test]
    fn project_write_manifest_roundtrip() {
        let dir = tempdir().unwrap();
        let manifest_path = dir.path().join("package.json");
        std::fs::write(&manifest_path, r#"{ "name": "original" }"#).unwrap();

        let project = Project::from_manifest_path(manifest_path).unwrap();

        let mut modified = project.manifest.clone();
        modified.name = Some("modified".to_string());
        project.write_manifest(&modified).unwrap();

        let reloaded = Project::from_manifest_path(project.manifest_path).unwrap();
        assert_eq!(reloaded.manifest.name.as_deref(), Some("modified"));
    }
}

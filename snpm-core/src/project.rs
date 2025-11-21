use crate::{Result, SnpmError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

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
    pub scripts: BTreeMap<String, String>,
}

#[derive(Debug)]
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
        let data =
            serde_json::to_string_pretty(manifest).map_err(|e| SnpmError::SerializeJson {
                path: self.manifest_path.clone(),
                reason: e.to_string(),
            })?;

        fs::write(&self.manifest_path, data).map_err(|source| SnpmError::WriteFile {
            path: self.manifest_path.clone(),
            source,
        })?;

        Ok(())
    }
}

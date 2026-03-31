use crate::{Project, Result, SnpmError};

#[derive(Clone)]
pub(super) struct PackageIdentity {
    pub(super) name: String,
    pub(super) version: String,
}

pub(super) fn publish_identity(project: &Project) -> Result<PackageIdentity> {
    let name = project
        .manifest
        .name
        .as_deref()
        .ok_or_else(|| SnpmError::ManifestInvalid {
            path: project.manifest_path.clone(),
            reason: "package.json must have a \"name\" field to publish".into(),
        })?;

    let version =
        project
            .manifest
            .version
            .as_deref()
            .ok_or_else(|| SnpmError::ManifestInvalid {
                path: project.manifest_path.clone(),
                reason: "package.json must have a \"version\" field to publish".into(),
            })?;

    Ok(PackageIdentity {
        name: name.to_string(),
        version: version.to_string(),
    })
}

pub(super) fn read_manifest_value(project: &Project) -> Result<serde_json::Value> {
    let manifest_json =
        std::fs::read_to_string(&project.manifest_path).map_err(|source| SnpmError::ReadFile {
            path: project.manifest_path.clone(),
            source,
        })?;

    serde_json::from_str(&manifest_json).map_err(|source| SnpmError::ParseJson {
        path: project.manifest_path.clone(),
        source,
    })
}

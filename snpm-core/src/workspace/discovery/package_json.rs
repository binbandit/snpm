use crate::project::WorkspacesField;
use crate::{Result, SnpmError};
use serde::Deserialize;
use std::fs;
use std::path::Path;

pub(super) fn read_package_json_workspaces(path: &Path) -> Result<Option<WorkspacesField>> {
    let data = fs::read_to_string(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    #[derive(Deserialize)]
    struct PartialManifest {
        workspaces: Option<WorkspacesField>,
    }

    let manifest: PartialManifest =
        serde_json::from_str(&data).map_err(|source| SnpmError::ParseJson {
            path: path.to_path_buf(),
            source,
        })?;

    Ok(manifest.workspaces)
}

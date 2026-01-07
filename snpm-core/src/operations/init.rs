use crate::project::Manifest;
use crate::{Result, SnpmError, console};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub fn init(root: &Path) -> Result<()> {
    let manifest_path = root.join("package.json");

    if manifest_path.is_file() {
        return Ok(());
    }

    let name = root
        .file_name()
        .and_then(|os| os.to_str())
        .unwrap_or("snpm-project")
        .to_string();

    let manifest = Manifest {
        name: Some(name),
        version: Some("0.1.0".to_string()),
        dependencies: BTreeMap::new(),
        dev_dependencies: BTreeMap::new(),
        scripts: BTreeMap::new(),
        pnpm: None,
        snpm: None,
        workspaces: None,
    };

    let data =
        serde_json::to_string_pretty(&manifest).map_err(|error| SnpmError::SerializeJson {
            path: manifest_path.clone(),
            reason: error.to_string(),
        })?;

    fs::write(&manifest_path, data).map_err(|source| SnpmError::WriteFile {
        path: manifest_path,
        source,
    })?;

    console::info("Created package.json");

    Ok(())
}

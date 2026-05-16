use crate::{Result, SnpmConfig, SnpmError};

use std::fs;

pub fn read_current(config: &SnpmConfig) -> Result<Option<String>> {
    let path = config.node_current_pointer_path();
    match fs::read_to_string(&path) {
        Ok(content) => {
            let value = content.trim().to_string();
            if value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(value))
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(SnpmError::ReadFile { path, source }),
    }
}

pub fn write_current(config: &SnpmConfig, version_with_v: &str) -> Result<()> {
    let path = config.node_current_pointer_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(&path, format!("{}\n", version_with_v.trim())).map_err(|source| {
        SnpmError::WriteFile {
            path: path.clone(),
            source,
        }
    })?;
    Ok(())
}

pub fn clear_current(config: &SnpmConfig) -> Result<()> {
    let path = config.node_current_pointer_path();
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(SnpmError::WriteFile { path, source }),
    }
}

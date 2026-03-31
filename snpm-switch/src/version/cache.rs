use crate::config;

use std::fs;

pub fn list_cached_versions() -> anyhow::Result<Vec<String>> {
    let versions_dir = config::versions_dir()?;

    if !versions_dir.is_dir() {
        return Ok(Vec::new());
    }

    let mut versions = Vec::new();

    for entry in fs::read_dir(&versions_dir)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir()
            && let Some(name) = path.file_name().and_then(|name| name.to_str())
        {
            versions.push(name.to_string());
        }
    }

    versions.sort();
    Ok(versions)
}

pub fn clear_cache() -> anyhow::Result<()> {
    let versions_dir = config::versions_dir()?;

    if versions_dir.is_dir() {
        fs::remove_dir_all(&versions_dir)?;
    }

    Ok(())
}

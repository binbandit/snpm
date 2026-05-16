use crate::{Result, SnpmConfig, SnpmError};

use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_ALIAS: &str = "default";

pub struct AliasEntry {
    pub name: String,
    pub target: String,
}

pub fn alias_path(config: &SnpmConfig, name: &str) -> Result<PathBuf> {
    if !is_valid_alias_name(name) {
        return Err(SnpmError::Internal {
            reason: format!("invalid Node alias name '{name}' (use letters, digits, _, -, /, .)"),
        });
    }
    Ok(config.node_aliases_dir().join(name))
}

pub fn read_alias(config: &SnpmConfig, name: &str) -> Result<Option<String>> {
    let path = match alias_path(config, name) {
        Ok(path) => path,
        Err(_) => return Ok(None),
    };

    match fs::read_to_string(&path) {
        Ok(content) => Ok(Some(content.trim().to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(SnpmError::ReadFile { path, source }),
    }
}

pub fn write_alias(config: &SnpmConfig, name: &str, target: &str) -> Result<PathBuf> {
    let path = alias_path(config, name)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(&path, format!("{}\n", target.trim())).map_err(|source| SnpmError::WriteFile {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

pub fn remove_alias(config: &SnpmConfig, name: &str) -> Result<bool> {
    let path = alias_path(config, name)?;
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(SnpmError::WriteFile { path, source }),
    }
}

pub fn list_aliases(config: &SnpmConfig) -> Result<Vec<AliasEntry>> {
    let aliases_dir = config.node_aliases_dir();
    let mut entries = Vec::new();

    if !aliases_dir.is_dir() {
        return Ok(entries);
    }

    walk_aliases(&aliases_dir, &aliases_dir, &mut entries)?;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

pub fn default_alias_name() -> &'static str {
    DEFAULT_ALIAS
}

fn walk_aliases(root: &Path, dir: &Path, entries: &mut Vec<AliasEntry>) -> Result<()> {
    let read = fs::read_dir(dir).map_err(|source| SnpmError::ReadFile {
        path: dir.to_path_buf(),
        source,
    })?;

    for entry in read {
        let entry = entry.map_err(|source| SnpmError::ReadFile {
            path: dir.to_path_buf(),
            source,
        })?;

        let path = entry.path();
        if path.is_dir() {
            walk_aliases(root, &path, entries)?;
            continue;
        }

        let Ok(relative) = path.strip_prefix(root) else {
            continue;
        };

        let name = relative.to_string_lossy().replace('\\', "/");
        let content = fs::read_to_string(&path).map_err(|source| SnpmError::ReadFile {
            path: path.clone(),
            source,
        })?;

        entries.push(AliasEntry {
            name,
            target: content.trim().to_string(),
        });
    }

    Ok(())
}

fn is_valid_alias_name(name: &str) -> bool {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return false;
    }
    if trimmed.starts_with('/') || trimmed.contains("..") {
        return false;
    }
    trimmed
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '/' | '.'))
}

#[cfg(test)]
mod tests {
    use super::is_valid_alias_name;

    #[test]
    fn accepts_simple_alias_names() {
        assert!(is_valid_alias_name("default"));
        assert!(is_valid_alias_name("work_project"));
        assert!(is_valid_alias_name("lts/iron"));
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(!is_valid_alias_name("../etc/passwd"));
        assert!(!is_valid_alias_name("/abs/path"));
        assert!(!is_valid_alias_name(""));
        assert!(!is_valid_alias_name("with space"));
    }
}

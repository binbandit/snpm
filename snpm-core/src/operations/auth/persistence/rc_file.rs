use crate::{Result, SnpmError};
use directories::BaseDirs;
use std::fs;
use std::path::{Path, PathBuf};

pub(super) fn rc_path() -> PathBuf {
    BaseDirs::new()
        .map(|dirs| dirs.home_dir().join(".snpmrc"))
        .unwrap_or_else(|| PathBuf::from(".snpmrc"))
}

pub(super) fn read_rc_file(path: &Path) -> Result<Vec<String>> {
    if !path.is_file() {
        return Ok(Vec::new());
    }

    fs::read_to_string(path)
        .map(|content| content.lines().map(String::from).collect())
        .map_err(|source| SnpmError::ReadFile {
            path: path.to_path_buf(),
            source,
        })
}

pub(super) fn write_rc_file(path: &Path, lines: &[String]) -> Result<()> {
    let content = if lines.is_empty() {
        String::new()
    } else {
        format!("{}\n", lines.join("\n"))
    };

    fs::write(path, content).map_err(|source| SnpmError::WriteFile {
        path: path.to_path_buf(),
        source,
    })
}

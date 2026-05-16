use crate::{Result, SnpmError};

use std::fs;
use std::path::{Path, PathBuf};

const PIN_FILES: [&str; 2] = [".node-version", ".nvmrc"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PinnedNode {
    pub spec: String,
    pub source: PinnedNodeSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PinnedNodeSource {
    File(PathBuf),
    EnginesNode(PathBuf),
}

pub fn discover_pinned(start: &Path) -> Result<Option<PinnedNode>> {
    if let Some(pinned) = walk_for_pin_files(start)? {
        return Ok(Some(pinned));
    }
    discover_engines_node(start)
}

fn walk_for_pin_files(start: &Path) -> Result<Option<PinnedNode>> {
    let mut current = Some(start);
    while let Some(dir) = current {
        for filename in PIN_FILES {
            let candidate = dir.join(filename);
            if let Some(spec) = read_pin_file(&candidate)? {
                return Ok(Some(PinnedNode {
                    spec,
                    source: PinnedNodeSource::File(candidate),
                }));
            }
        }
        current = dir.parent();
    }
    Ok(None)
}

fn read_pin_file(path: &Path) -> Result<Option<String>> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(parse_pin_file_contents(&content)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(SnpmError::ReadFile {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn parse_pin_file_contents(content: &str) -> Option<String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        return Some(trimmed.to_string());
    }
    None
}

fn discover_engines_node(start: &Path) -> Result<Option<PinnedNode>> {
    let mut current = Some(start);
    while let Some(dir) = current {
        let manifest_path = dir.join("package.json");
        if manifest_path.is_file()
            && let Some(spec) = read_engines_node(&manifest_path)?
        {
            return Ok(Some(PinnedNode {
                spec,
                source: PinnedNodeSource::EnginesNode(manifest_path),
            }));
        }
        current = dir.parent();
    }
    Ok(None)
}

fn read_engines_node(manifest_path: &Path) -> Result<Option<String>> {
    let content = match fs::read_to_string(manifest_path) {
        Ok(content) => content,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(SnpmError::ReadFile {
                path: manifest_path.to_path_buf(),
                source,
            });
        }
    };

    let value: serde_json::Value = match serde_json::from_str(&content) {
        Ok(value) => value,
        Err(_) => return Ok(None),
    };

    let raw = value
        .get("engines")
        .and_then(|engines| engines.get("node"))
        .and_then(|node| node.as_str())
        .map(|s| s.trim().to_string());

    Ok(raw.filter(|spec| !spec.is_empty() && spec != "*"))
}

#[cfg(test)]
mod tests {
    use super::{PinnedNodeSource, discover_pinned, parse_pin_file_contents};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn pin_file_skips_comments_and_blanks() {
        let content = "# managed by snpm\n\n  20.10.0\n";
        assert_eq!(parse_pin_file_contents(content).as_deref(), Some("20.10.0"));
    }

    #[test]
    fn discovers_node_version_file_in_ancestor() {
        let root = tempdir().unwrap();
        let nested = root.path().join("packages").join("api");
        fs::create_dir_all(&nested).unwrap();
        fs::write(root.path().join(".node-version"), "v20.10.0\n").unwrap();

        let pinned = discover_pinned(&nested).unwrap().unwrap();
        assert_eq!(pinned.spec, "v20.10.0");
        assert!(matches!(pinned.source, PinnedNodeSource::File(_)));
    }

    #[test]
    fn falls_back_to_engines_node() {
        let root = tempdir().unwrap();
        fs::write(
            root.path().join("package.json"),
            r#"{"name":"x","engines":{"node":">=18 <21"}}"#,
        )
        .unwrap();

        let pinned = discover_pinned(root.path()).unwrap().unwrap();
        assert_eq!(pinned.spec, ">=18 <21");
        assert!(matches!(pinned.source, PinnedNodeSource::EnginesNode(_)));
    }

    #[test]
    fn returns_none_when_nothing_pinned() {
        let root = tempdir().unwrap();
        fs::write(root.path().join("package.json"), r#"{"name":"x"}"#).unwrap();
        assert!(discover_pinned(root.path()).unwrap().is_none());
    }
}

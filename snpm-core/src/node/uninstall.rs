use crate::{Result, SnpmConfig, SnpmError};

use super::aliases::{self, AliasEntry};
use super::current;

use std::fs;
use std::path::PathBuf;

pub struct UninstallSummary {
    pub version: String,
    pub removed_dir: Option<PathBuf>,
    pub cleared_current: bool,
    pub removed_aliases: Vec<String>,
}

pub fn uninstall_version(config: &SnpmConfig, version_with_v: &str) -> Result<UninstallSummary> {
    let version_dir = config.node_version_dir(version_with_v);

    let removed_dir = if version_dir.exists() {
        fs::remove_dir_all(&version_dir).map_err(|source| SnpmError::WriteFile {
            path: version_dir.clone(),
            source,
        })?;
        Some(version_dir)
    } else {
        None
    };

    let cleared_current = match current::read_current(config)? {
        Some(active) if active == version_with_v => {
            current::clear_current(config)?;
            true
        }
        _ => false,
    };

    let removed_aliases = clear_matching_aliases(config, version_with_v)?;

    Ok(UninstallSummary {
        version: version_with_v.to_string(),
        removed_dir,
        cleared_current,
        removed_aliases,
    })
}

pub fn list_installed_versions(config: &SnpmConfig) -> Result<Vec<String>> {
    let versions_dir = config.node_versions_dir();
    let mut versions = Vec::new();

    if !versions_dir.is_dir() {
        return Ok(versions);
    }

    let read = fs::read_dir(&versions_dir).map_err(|source| SnpmError::ReadFile {
        path: versions_dir.clone(),
        source,
    })?;

    for entry in read {
        let entry = entry.map_err(|source| SnpmError::ReadFile {
            path: versions_dir.clone(),
            source,
        })?;

        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };

        if name.starts_with('.') || name.ends_with(".tmp") {
            continue;
        }

        if !path.join(".snpm_complete").is_file() {
            continue;
        }

        versions.push(name.to_string());
    }

    versions.sort_by(|a, b| natural_compare_versions(b, a));
    Ok(versions)
}

pub fn is_version_installed(config: &SnpmConfig, version_with_v: &str) -> bool {
    let dir = config.node_version_dir(version_with_v);
    dir.join(".snpm_complete").is_file() && super::install::node_binary_path(&dir).is_file()
}

fn clear_matching_aliases(config: &SnpmConfig, version_with_v: &str) -> Result<Vec<String>> {
    let mut cleared = Vec::new();
    for entry in aliases::list_aliases(config)? {
        if alias_points_at(&entry, version_with_v) {
            aliases::remove_alias(config, &entry.name)?;
            cleared.push(entry.name);
        }
    }
    Ok(cleared)
}

fn alias_points_at(entry: &AliasEntry, version_with_v: &str) -> bool {
    let target = entry.target.trim();
    target == version_with_v || target == version_with_v.trim_start_matches('v')
}

fn natural_compare_versions(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |label: &str| {
        let candidate = label.strip_prefix('v').unwrap_or(label);
        snpm_semver::Version::parse(candidate).ok()
    };

    match (parse(a), parse(b)) {
        (Some(va), Some(vb)) => va.cmp(&vb),
        (Some(_), None) => std::cmp::Ordering::Greater,
        (None, Some(_)) => std::cmp::Ordering::Less,
        (None, None) => a.cmp(b),
    }
}

#[cfg(test)]
mod tests {
    use super::{alias_points_at, natural_compare_versions};
    use crate::node::aliases::AliasEntry;

    #[test]
    fn alias_matches_with_or_without_v() {
        let entry = AliasEntry {
            name: "default".into(),
            target: "v20.10.0".into(),
        };
        assert!(alias_points_at(&entry, "v20.10.0"));

        let entry = AliasEntry {
            name: "default".into(),
            target: "20.10.0".into(),
        };
        assert!(alias_points_at(&entry, "v20.10.0"));
    }

    #[test]
    fn versions_sort_newest_first() {
        let mut versions = vec![
            "v18.19.0".to_string(),
            "v20.10.0".to_string(),
            "v21.6.0".to_string(),
        ];
        versions.sort_by(|a, b| natural_compare_versions(b, a));
        assert_eq!(versions, vec!["v21.6.0", "v20.10.0", "v18.19.0"]);
    }
}

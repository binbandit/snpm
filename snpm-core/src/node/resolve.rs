use crate::{Result, SnpmConfig, SnpmError};

use super::aliases;
use super::index::{NodeRelease, fetch_index, read_cached_index};

use snpm_semver::{RangeSet, Version};

const MAX_ALIAS_DEPTH: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedNodeVersion {
    pub normalized: String,
    pub source: ResolutionSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolutionSource {
    Exact,
    Alias { name: String, target: String },
    Remote { selector: String },
}

impl ResolvedNodeVersion {
    pub fn version_with_v(&self) -> &str {
        &self.normalized
    }
}

pub fn normalize_version(input: &str) -> Option<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let candidate = trimmed.strip_prefix('v').unwrap_or(trimmed);
    let parsed = Version::parse(candidate).ok()?;
    Some(format!("v{}", parsed))
}

pub fn is_lts_selector(spec: &str) -> bool {
    let lower = spec.trim().to_ascii_lowercase();
    lower == "lts" || lower == "--lts" || lower.starts_with("lts/")
}

pub fn lts_codename(spec: &str) -> Option<String> {
    let lower = spec.trim().to_ascii_lowercase();
    let suffix = lower.strip_prefix("lts/")?;
    if suffix.is_empty() || suffix == "*" || suffix == "latest" {
        None
    } else {
        Some(suffix.to_string())
    }
}

pub async fn resolve_spec(
    config: &SnpmConfig,
    spec: &str,
    allow_network: bool,
) -> Result<ResolvedNodeVersion> {
    resolve_with_depth(config, spec, allow_network, 0).await
}

#[async_recursion::async_recursion]
async fn resolve_with_depth(
    config: &SnpmConfig,
    spec: &str,
    allow_network: bool,
    depth: usize,
) -> Result<ResolvedNodeVersion> {
    if depth > MAX_ALIAS_DEPTH {
        return Err(SnpmError::Internal {
            reason: format!("alias chain exceeds {MAX_ALIAS_DEPTH} hops resolving '{spec}'"),
        });
    }

    let trimmed = spec.trim();
    if trimmed.is_empty() {
        return Err(SnpmError::Internal {
            reason: "empty Node version selector".into(),
        });
    }

    if let Some(target) = aliases::read_alias(config, trimmed)? {
        let resolved = resolve_with_depth(config, &target, allow_network, depth + 1).await?;
        return Ok(ResolvedNodeVersion {
            normalized: resolved.normalized,
            source: ResolutionSource::Alias {
                name: trimmed.to_string(),
                target,
            },
        });
    }

    if let Some(normalized) = normalize_version(trimmed) {
        return Ok(ResolvedNodeVersion {
            normalized,
            source: ResolutionSource::Exact,
        });
    }

    let releases = load_releases(config, allow_network).await?;
    let normalized = match_remote(trimmed, &releases)?;

    Ok(ResolvedNodeVersion {
        normalized,
        source: ResolutionSource::Remote {
            selector: trimmed.to_string(),
        },
    })
}

async fn load_releases(config: &SnpmConfig, allow_network: bool) -> Result<Vec<NodeRelease>> {
    if allow_network {
        fetch_index(config, false).await
    } else {
        read_cached_index(config).ok_or_else(|| SnpmError::Internal {
            reason: "offline: Node release index not cached yet — run `snpm node ls-remote` first"
                .into(),
        })
    }
}

fn match_remote(spec: &str, releases: &[NodeRelease]) -> Result<String> {
    let lower = spec.to_ascii_lowercase();

    if lower == "latest" || lower == "current" || lower == "node" || lower == "stable" {
        return latest_version(releases);
    }

    if is_lts_selector(&lower) {
        let codename = lts_codename(&lower);
        return latest_lts(releases, codename.as_deref());
    }

    if let Some(version) = match_partial(spec, releases) {
        return Ok(version);
    }

    if let Some(version) = match_range(spec, releases) {
        return Ok(version);
    }

    Err(SnpmError::Internal {
        reason: format!("could not resolve Node version selector '{spec}'"),
    })
}

fn latest_version(releases: &[NodeRelease]) -> Result<String> {
    releases
        .iter()
        .map(|release| release.version.clone())
        .next()
        .ok_or_else(|| SnpmError::Internal {
            reason: "Node release index is empty".into(),
        })
}

fn latest_lts(releases: &[NodeRelease], codename: Option<&str>) -> Result<String> {
    let needle = codename.map(|name| name.to_ascii_lowercase());

    for release in releases {
        let Some(release_codename) = release.lts.codename() else {
            continue;
        };

        if let Some(target) = needle.as_deref() {
            if release_codename.to_ascii_lowercase() == target {
                return Ok(release.version.clone());
            }
        } else {
            return Ok(release.version.clone());
        }
    }

    let label = needle
        .map(|name| format!("lts/{name}"))
        .unwrap_or_else(|| "lts/*".to_string());
    Err(SnpmError::Internal {
        reason: format!("no LTS release matches {label}"),
    })
}

fn match_partial(spec: &str, releases: &[NodeRelease]) -> Option<String> {
    let trimmed = spec.strip_prefix('v').unwrap_or(spec);
    let parts: Vec<&str> = trimmed.split('.').collect();

    if !parts
        .iter()
        .all(|part| part.chars().all(|c| c.is_ascii_digit()))
    {
        return None;
    }

    let parsed_parts: Vec<u64> = parts.iter().filter_map(|p| p.parse().ok()).collect();
    if parsed_parts.len() != parts.len() || parsed_parts.is_empty() {
        return None;
    }

    if parsed_parts.len() == 3 {
        return None;
    }

    let mut best: Option<Version> = None;
    let mut best_label: Option<String> = None;

    for release in releases {
        let candidate = release
            .version
            .strip_prefix('v')
            .unwrap_or(&release.version);
        let Ok(parsed) = Version::parse(candidate) else {
            continue;
        };

        let matches_prefix = match parsed_parts.len() {
            1 => parsed.major == parsed_parts[0],
            2 => parsed.major == parsed_parts[0] && parsed.minor == parsed_parts[1],
            _ => false,
        };

        if !matches_prefix {
            continue;
        }

        if best.as_ref().is_none_or(|current| parsed > *current) {
            best = Some(parsed);
            best_label = Some(release.version.clone());
        }
    }

    best_label
}

fn match_range(spec: &str, releases: &[NodeRelease]) -> Option<String> {
    let range = RangeSet::parse(spec).ok()?;
    let mut best: Option<Version> = None;
    let mut best_label: Option<String> = None;

    for release in releases {
        let candidate = release
            .version
            .strip_prefix('v')
            .unwrap_or(&release.version);
        let Ok(parsed) = Version::parse(candidate) else {
            continue;
        };

        if !range.matches(&parsed) {
            continue;
        }

        if best.as_ref().is_none_or(|current| parsed > *current) {
            best = Some(parsed);
            best_label = Some(release.version.clone());
        }
    }

    best_label
}

#[cfg(test)]
mod tests {
    use super::{is_lts_selector, lts_codename, match_partial, match_range, normalize_version};
    use crate::node::index::{LtsField, NodeRelease};

    fn release(version: &str, lts: Option<&str>) -> NodeRelease {
        NodeRelease {
            version: version.to_string(),
            date: "2024-01-01".into(),
            files: vec![],
            npm: None,
            lts: match lts {
                Some(name) => LtsField::Codename(name.to_string()),
                None => LtsField::None(false),
            },
            security: false,
        }
    }

    #[test]
    fn normalizes_exact_versions() {
        assert_eq!(normalize_version("20.10.0").as_deref(), Some("v20.10.0"));
        assert_eq!(normalize_version("v20.10.0").as_deref(), Some("v20.10.0"));
        assert_eq!(
            normalize_version("v20.10.0-rc.1").as_deref(),
            Some("v20.10.0-rc.1")
        );
        assert!(normalize_version("20.10").is_none());
        assert!(normalize_version("").is_none());
    }

    #[test]
    fn classifies_lts_selectors() {
        assert!(is_lts_selector("lts"));
        assert!(is_lts_selector("--lts"));
        assert!(is_lts_selector("lts/iron"));
        assert!(!is_lts_selector("20"));
    }

    #[test]
    fn parses_lts_codenames() {
        assert_eq!(lts_codename("lts/Iron").as_deref(), Some("iron"));
        assert!(lts_codename("lts").is_none());
        assert!(lts_codename("lts/*").is_none());
    }

    #[test]
    fn partial_matches_pick_latest_in_series() {
        let releases = vec![
            release("v20.10.0", None),
            release("v20.5.1", None),
            release("v18.19.0", Some("Hydrogen")),
        ];
        assert_eq!(match_partial("20", &releases).as_deref(), Some("v20.10.0"));
        assert_eq!(match_partial("20.5", &releases).as_deref(), Some("v20.5.1"));
        assert!(match_partial("21", &releases).is_none());
    }

    #[test]
    fn range_matches_pick_latest_satisfying() {
        let releases = vec![
            release("v20.10.0", None),
            release("v20.5.1", None),
            release("v18.19.0", None),
        ];
        assert_eq!(match_range("^20", &releases).as_deref(), Some("v20.10.0"));
        assert_eq!(
            match_range(">=18 <20", &releases).as_deref(),
            Some("v18.19.0")
        );
    }
}

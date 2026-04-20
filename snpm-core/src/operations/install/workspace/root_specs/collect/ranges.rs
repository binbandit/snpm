use crate::{Result, SnpmError, Workspace};

use snpm_semver::RangeSet;
use std::collections::BTreeMap;
use std::path::Path;

pub fn insert_workspace_root_dep(
    combined: &mut BTreeMap<String, String>,
    workspace_root: &Path,
    declaring_package_root: &Path,
    name: &str,
    range: &str,
) -> Result<()> {
    let resolved_range = resolve_workspace_range(name, range, declaring_package_root)?;

    if let Some(existing) = combined.get(name) {
        if let Some(merged) = merge_workspace_ranges(existing, &resolved_range) {
            combined.insert(name.to_string(), merged);
        } else {
            return Err(SnpmError::WorkspaceConfig {
                path: workspace_root.to_path_buf(),
                reason: format!(
                    "dependency {name} has conflicting ranges {existing} and {resolved_range} across workspace projects"
                ),
            });
        }
    } else {
        combined.insert(name.to_string(), resolved_range);
    }

    Ok(())
}

pub fn conflicting_range_error<T>(
    workspace: &Workspace,
    name: &str,
    existing: &str,
    incoming: &str,
) -> Result<T> {
    Err(SnpmError::WorkspaceConfig {
        path: workspace.root.clone(),
        reason: format!(
            "dependency {name} has conflicting ranges {existing} and {incoming} across workspace projects"
        ),
    })
}

fn resolve_workspace_range(
    name: &str,
    range: &str,
    declaring_package_root: &Path,
) -> Result<String> {
    let Some(file_path) = range.strip_prefix("file:") else {
        return Ok(range.to_string());
    };

    let path = Path::new(file_path);
    if path.is_absolute() {
        return Ok(range.to_string());
    }

    let absolute = declaring_package_root.join(path);
    let canonical = absolute
        .canonicalize()
        .map_err(|error| SnpmError::ResolutionFailed {
            name: name.to_string(),
            range: range.to_string(),
            reason: format!(
                "Failed to resolve file path {}: {}",
                absolute.display(),
                error
            ),
        })?;

    Ok(format!("file:{}", canonical.display()))
}

fn merge_workspace_ranges(existing: &str, incoming: &str) -> Option<String> {
    if existing == incoming {
        return Some(existing.to_string());
    }

    if existing.starts_with("file:") || incoming.starts_with("file:") {
        return None;
    }

    let existing_ranges = RangeSet::parse(existing).ok()?;
    let incoming_ranges = RangeSet::parse(incoming).ok()?;
    let existing_floor_raw = range_floor(existing)?;
    let incoming_floor_raw = range_floor(incoming)?;
    let existing_floor = snpm_semver::parse_version(&existing_floor_raw).ok()?;
    let incoming_floor = snpm_semver::parse_version(&incoming_floor_raw).ok()?;

    let existing_accepts_incoming = existing_ranges.matches(&incoming_floor);
    let incoming_accepts_existing = incoming_ranges.matches(&existing_floor);

    let merged = match (existing_accepts_incoming, incoming_accepts_existing) {
        (true, true) => {
            choose_more_specific(existing, incoming, &existing_floor_raw, &incoming_floor_raw)
        }
        (true, false) => incoming,
        (false, true) => existing,
        (false, false) => {
            choose_more_specific(existing, incoming, &existing_floor_raw, &incoming_floor_raw)
        }
    };

    Some(merged.to_string())
}

fn range_floor(range: &str) -> Option<String> {
    for token in range.split(|ch: char| ch.is_whitespace() || ch == ',' || ch == '|') {
        let token = token.trim().trim_start_matches(['^', '~', '=', '>', '<']);
        if token.is_empty() {
            continue;
        }

        if let Some(normalized) = normalize_floor_version(token) {
            return Some(normalized);
        }
    }

    None
}

fn normalize_floor_version(token: &str) -> Option<String> {
    let token = token.trim();
    if token.is_empty() {
        return None;
    }

    if snpm_semver::parse_version(token).is_ok() {
        return Some(token.to_string());
    }

    let parts: Vec<&str> = token.split('.').collect();
    let normalized = match parts.as_slice() {
        [major] if major.parse::<u64>().is_ok() => format!("{major}.0.0"),
        [major, minor] if major.parse::<u64>().is_ok() && minor.parse::<u64>().is_ok() => {
            format!("{major}.{minor}.0")
        }
        _ => return None,
    };

    snpm_semver::parse_version(&normalized).ok()?;
    Some(normalized)
}

fn choose_more_specific<'a>(
    existing: &'a str,
    incoming: &'a str,
    existing_floor: &str,
    incoming_floor: &str,
) -> &'a str {
    let existing_floor = snpm_semver::parse_version(existing_floor).ok();
    let incoming_floor = snpm_semver::parse_version(incoming_floor).ok();

    match (existing_floor, incoming_floor) {
        (Some(existing_floor), Some(incoming_floor)) if incoming_floor > existing_floor => incoming,
        (Some(existing_floor), Some(incoming_floor)) if existing_floor > incoming_floor => existing,
        _ => {
            if specificity_rank(incoming) > specificity_rank(existing) {
                incoming
            } else {
                existing
            }
        }
    }
}

fn specificity_rank(range: &str) -> u8 {
    let trimmed = range.trim();
    if is_exact_version(trimmed) {
        3
    } else if trimmed.starts_with('~') {
        2
    } else if trimmed.starts_with('^') {
        1
    } else {
        0
    }
}

fn is_exact_version(range: &str) -> bool {
    let candidate = range.trim_start_matches('=');
    snpm_semver::parse_version(candidate).is_ok()
}

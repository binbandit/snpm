use crate::{Result, SnpmError, Workspace};
use snpm_semver::RangeSet;

pub fn validate_workspace_spec(workspace: &Workspace, name: &str, spec: &str) -> Result<()> {
    let project = workspace
        .projects
        .iter()
        .find(|candidate| candidate.manifest.name.as_deref() == Some(name))
        .ok_or_else(|| SnpmError::WorkspaceConfig {
            path: workspace.root.clone(),
            reason: format!("workspace dependency {name} not found in workspace projects"),
        })?;

    let version =
        project
            .manifest
            .version
            .as_deref()
            .ok_or_else(|| SnpmError::WorkspaceConfig {
                path: workspace.root.clone(),
                reason: format!("workspace dependency {name} has no version in its package.json"),
            })?;

    let range_str = normalize_workspace_spec(name, spec, version)?;
    if range_str.is_empty() {
        return Ok(());
    }

    let ranges = RangeSet::parse(&range_str).map_err(|error| SnpmError::Semver {
        value: format!("{}@{}", name, range_str),
        reason: error.to_string(),
    })?;

    let version_parsed =
        snpm_semver::parse_version(version).map_err(|error| SnpmError::Semver {
            value: format!("{}@{}", name, version),
            reason: error.to_string(),
        })?;

    if ranges.matches(&version_parsed) {
        Ok(())
    } else {
        Err(SnpmError::WorkspaceConfig {
            path: workspace.root.clone(),
            reason: format!(
                "workspace dependency {name} with spec {spec} is not satisfied by local version {version}"
            ),
        })
    }
}

fn normalize_workspace_spec(name: &str, spec: &str, version: &str) -> Result<String> {
    let Some(suffix) = spec.strip_prefix("workspace:") else {
        return Err(SnpmError::WorkspaceConfig {
            path: std::path::PathBuf::new(),
            reason: format!("workspace dependency {name} must use workspace: spec, got {spec}"),
        });
    };

    let trimmed = suffix.trim();
    if trimmed.is_empty() || trimmed == "*" {
        return Ok(String::new());
    }

    let normalized = match trimmed {
        "^" => format!("^{}", version),
        "~" => format!("~{}", version),
        other => other.to_string(),
    };

    Ok(normalized)
}

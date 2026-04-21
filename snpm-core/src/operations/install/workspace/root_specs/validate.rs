use crate::{Result, SnpmError, Workspace};
use snpm_semver::RangeSet;

pub fn validate_workspace_spec(workspace: &Workspace, name: &str, spec: &str) -> Result<()> {
    let project = workspace
        .project_by_name(name)
        .ok_or_else(|| SnpmError::WorkspaceConfig {
            path: workspace.root.clone(),
            reason: format!("workspace dependency {name} not found in workspace projects"),
        })?;

    let Some(version) = project.manifest.version.as_deref() else {
        return Ok(());
    };

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

pub(crate) fn is_local_workspace_dependency(
    workspace: &Workspace,
    name: &str,
    spec: &str,
) -> Result<bool> {
    let Some(project) = workspace.project_by_name(name) else {
        return Ok(false);
    };

    if spec.starts_with("workspace:") {
        validate_workspace_spec(workspace, name, spec)?;
        return Ok(true);
    }

    let Some(version) = project.manifest.version.as_deref() else {
        return Ok(false);
    };

    let ranges = match RangeSet::parse(spec) {
        Ok(ranges) => ranges,
        Err(_) => return Ok(false),
    };

    let version_parsed =
        snpm_semver::parse_version(version).map_err(|error| SnpmError::Semver {
            value: format!("{}@{}", name, version),
            reason: error.to_string(),
        })?;

    Ok(ranges.matches(&version_parsed))
}

pub(crate) fn normalize_workspace_spec(name: &str, spec: &str, version: &str) -> Result<String> {
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

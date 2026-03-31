use crate::{Result, SnpmError, Workspace};

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
        if existing != &resolved_range {
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

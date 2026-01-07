use crate::console;
use crate::operations::install::utils::InstallResult;
use crate::operations::install::{InstallOptions, install};
use crate::{Result, SnpmConfig, SnpmError, Workspace};
use snpm_semver::{RangeSet, Version};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;
use std::time::Instant;

use super::manifest::apply_specs;

/// Install dependencies for an entire workspace at once.
/// This resolves and downloads packages once, then links to all member projects.
pub async fn install_workspace(
    config: &SnpmConfig,
    workspace: &mut Workspace,
    include_dev: bool,
    frozen_lockfile: bool,
    force: bool,
) -> Result<InstallResult> {
    let started = Instant::now();

    if workspace.projects.is_empty() {
        return Ok(InstallResult {
            package_count: 0,
            elapsed_seconds: 0.0,
        });
    }

    // Use the first project as the "root" for resolution purposes
    let project = &mut workspace.projects[0];

    let options = InstallOptions {
        requested: Vec::new(),
        dev: false,
        include_dev,
        frozen_lockfile,
        force,
        silent_summary: true,
    };

    // Run install for the first project - this resolves all workspace deps
    let result = install(config, project, options.clone()).await?;

    // Now link to remaining workspace projects
    for project in workspace.projects.iter().skip(1) {
        let node_modules = project.root.join("node_modules");

        // Collect this project's local workspace deps
        let mut local_dependencies = BTreeSet::new();
        let mut local_development_dependencies = BTreeSet::new();

        for (name, value) in project.manifest.dependencies.iter() {
            if value.starts_with("workspace:") {
                local_dependencies.insert(name.clone());
            }
        }

        for (name, value) in project.manifest.dev_dependencies.iter() {
            if value.starts_with("workspace:") {
                local_development_dependencies.insert(name.clone());
            }
        }

        link_local_workspace_deps(
            project,
            Some(workspace),
            &local_dependencies,
            &local_development_dependencies,
            include_dev,
        )?;

        // Create node_modules if it doesn't exist (for projects with only workspace deps)
        if !node_modules.exists() {
            fs::create_dir_all(&node_modules).map_err(|source| SnpmError::WriteFile {
                path: node_modules.clone(),
                source,
            })?;
        }
    }

    let elapsed = started.elapsed();
    let seconds = elapsed.as_secs_f32();

    console::summary(result.package_count, seconds);

    Ok(InstallResult {
        package_count: result.package_count,
        elapsed_seconds: seconds,
    })
}

pub fn collect_workspace_root_deps(
    workspace: &Workspace,
    include_dev: bool,
) -> Result<BTreeMap<String, String>> {
    let mut combined = BTreeMap::new();

    for member in workspace.projects.iter() {
        let mut local = BTreeSet::new();
        let dependencies = apply_specs(
            &member.manifest.dependencies,
            Some(workspace),
            None,
            &mut local,
            None,
        )?;

        for (name, range) in dependencies.iter() {
            insert_workspace_root_dep(&mut combined, &workspace.root, name, range)?;
        }

        if include_dev {
            let mut local_development = BTreeSet::new();
            let development_dependencies = apply_specs(
                &member.manifest.dev_dependencies,
                Some(workspace),
                None,
                &mut local_development,
                None,
            )?;

            for (name, range) in development_dependencies.iter() {
                insert_workspace_root_dep(&mut combined, &workspace.root, name, range)?;
            }
        }
    }

    Ok(combined)
}

pub fn insert_workspace_root_dep(
    combined: &mut BTreeMap<String, String>,
    root: &Path,
    name: &str,
    range: &str,
) -> Result<()> {
    if let Some(existing) = combined.get(name) {
        if existing != range {
            return Err(SnpmError::WorkspaceConfig {
                path: root.to_path_buf(),
                reason: format!(
                    "dependency {name} has conflicting ranges {existing} and {range} across workspace projects"
                ),
            });
        }
    } else {
        combined.insert(name.to_string(), range.to_string());
    }

    Ok(())
}

pub fn validate_workspace_spec(workspace: &Workspace, name: &str, spec: &str) -> Result<()> {
    let project = workspace
        .projects
        .iter()
        .find(|p| p.manifest.name.as_deref() == Some(name))
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

    let suffix = &spec["workspace:".len()..];
    let trimmed = suffix.trim();

    if trimmed.is_empty() || trimmed == "*" {
        return Ok(());
    }

    let range_str = match trimmed {
        "^" => format!("^{}", version),
        "~" => format!("~{}", version),
        other => other.to_string(),
    };

    let ranges = RangeSet::parse(&range_str).map_err(|error| SnpmError::Semver {
        value: format!("{}@{}", name, range_str),
        reason: error.to_string(),
    })?;

    let version_parsed = Version::parse(version).map_err(|error| SnpmError::Semver {
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

pub fn link_local_workspace_deps(
    project: &crate::Project,
    workspace: Option<&Workspace>,
    local_deps: &BTreeSet<String>,
    local_dev_deps: &BTreeSet<String>,
    include_dev: bool,
) -> Result<()> {
    if local_deps.is_empty() && local_dev_deps.is_empty() {
        return Ok(());
    }

    let workspace_reference = match workspace {
        Some(w) => w,
        None => {
            return Err(SnpmError::WorkspaceConfig {
                path: project.root.clone(),
                reason: "workspace: protocol used but no workspace configuration found".into(),
            });
        }
    };

    let node_modules = project.root.join("node_modules");

    for name in local_deps.iter().chain(local_dev_deps.iter()) {
        let only_dev = local_dev_deps.contains(name) && !local_deps.contains(name);
        if !include_dev && only_dev {
            continue;
        }

        let source_project = workspace_reference
            .projects
            .iter()
            .find(|p| p.manifest.name.as_deref() == Some(name.as_str()))
            .ok_or_else(|| SnpmError::WorkspaceConfig {
                path: workspace_reference.root.clone(),
                reason: format!("workspace dependency {name} not found in workspace projects"),
            })?;

        let dest = node_modules.join(name);

        // Ensure parent directory exists (for scoped packages like @scope/pkg)
        if let Some(parent) = dest.parent()
            && !parent.exists()
        {
            fs::create_dir_all(parent).map_err(|source| SnpmError::WriteFile {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        if dest.exists() {
            if dest.is_dir() {
                fs::remove_dir_all(&dest)
            } else {
                fs::remove_file(&dest)
            }
            .map_err(|source| SnpmError::WriteFile {
                path: dest.clone(),
                source,
            })?;
        }

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(&source_project.root, &dest).map_err(|source| SnpmError::WriteFile {
                path: dest.clone(),
                source,
            })?;
        }

        #[cfg(windows)]
        {
            use std::os::windows::fs::symlink_dir;
            symlink_dir(&source_project.root, &dest).map_err(|source| SnpmError::WriteFile {
                path: dest.clone(),
                source,
            })?;
        }
    }

    Ok(())
}

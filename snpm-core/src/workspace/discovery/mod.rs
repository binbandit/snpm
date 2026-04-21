mod config;
mod package_json;
mod projects;

use super::types::{Workspace, WorkspaceConfig};
use crate::Result;
use std::path::Path;

use config::{
    empty_workspace_config, merge_catalog_entries, merge_snpm_catalog, merge_yarn_catalog,
    read_config,
};
use package_json::read_package_json_workspaces;
use projects::load_projects;

pub fn discover_workspace(start: &Path) -> Result<Option<Workspace>> {
    let mut current = Some(start);

    while let Some(directory) = current {
        if let Some(workspace) = try_load_workspace(directory)? {
            return Ok(Some(workspace));
        }

        current = directory.parent();
    }

    Ok(None)
}

fn try_load_workspace(dir: &Path) -> Result<Option<Workspace>> {
    let yaml_path = workspace_yaml_path(dir);
    let package_json_path = dir.join("package.json");
    let package_json_workspaces = if package_json_path.is_file() {
        read_package_json_workspaces(&package_json_path)?
    } else {
        None
    };

    if yaml_path.is_none() && package_json_workspaces.is_none() {
        return Ok(None);
    }

    let root = dir.to_path_buf();
    let mut config = match yaml_path {
        Some(path) => read_config(&path)?,
        None => empty_workspace_config(),
    };

    let include_root_project = package_json_path.is_file();

    if let Some((patterns, catalog, catalogs)) =
        package_json_workspaces.map(|workspaces| workspaces.into_parts())
    {
        merge_package_patterns(&mut config, patterns);
        merge_catalog_entries(&mut config, catalog, catalogs);
    }

    merge_snpm_catalog(&root, &mut config)?;
    merge_yarn_catalog(&root, &mut config)?;
    let projects = load_projects(&root, &config, include_root_project)?;

    Ok(Some(Workspace {
        root,
        projects,
        config,
    }))
}

fn workspace_yaml_path(dir: &Path) -> Option<std::path::PathBuf> {
    let snpm_path = dir.join("snpm-workspace.yaml");
    if snpm_path.is_file() {
        return Some(snpm_path);
    }

    let pnpm_path = dir.join("pnpm-workspace.yaml");
    pnpm_path.is_file().then_some(pnpm_path)
}

fn merge_package_patterns(config: &mut WorkspaceConfig, patterns: Vec<String>) {
    for pattern in patterns {
        if !config.packages.contains(&pattern) {
            config.packages.push(pattern);
        }
    }
}

#[cfg(test)]
mod tests;

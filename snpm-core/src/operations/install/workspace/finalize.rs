use crate::console;
use crate::lifecycle;
use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Result, SnpmConfig, SnpmError, Workspace};

use rayon::prelude::*;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::super::utils::{
    InstallScenario, IntegrityState, build_workspace_integrity_state, can_any_scripts_run,
    capture_workspace_layout_state, compute_project_patch_hash, write_integrity_path,
};
use super::linking::{
    link_project_dependencies, link_store_dependencies, populate_virtual_store,
    rebuild_virtual_store_paths,
};
use super::patches::apply_workspace_patches;
use super::resolution::write_workspace_integrity;

pub(super) fn finalize_workspace_install(
    config: &SnpmConfig,
    workspace: &Workspace,
    graph: &ResolutionGraph,
    store_paths_map: &BTreeMap<PackageId, PathBuf>,
    include_dev: bool,
    scenario: InstallScenario,
) -> Result<Vec<String>> {
    let shared_virtual_store = workspace.root.join(".snpm");
    fs::create_dir_all(&shared_virtual_store).map_err(|source| SnpmError::WriteFile {
        path: shared_virtual_store.clone(),
        source,
    })?;

    let virtual_store_paths = if matches!(scenario, InstallScenario::Hot) {
        console::step("Validating workspace structure");
        rebuild_virtual_store_paths(&shared_virtual_store, graph)?
    } else {
        console::step("Linking workspace");
        populate_virtual_store(
            &shared_virtual_store,
            graph,
            store_paths_map,
            config,
            workspace,
        )?
    };

    link_store_dependencies(&virtual_store_paths, graph)?;

    workspace.projects.par_iter().try_for_each(|project| {
        link_project_dependencies(project, workspace, graph, &virtual_store_paths, include_dev)
    })?;

    let patches_applied = apply_workspace_patches(workspace, store_paths_map)?;
    if patches_applied > 0 {
        console::verbose(&format!("applied {} workspace patches", patches_applied));
    }

    let blocked_scripts = run_workspace_scripts(config, workspace)?;
    let workspace_integrity = build_workspace_integrity_state(workspace, graph)?;
    write_workspace_integrity(&workspace.root, &workspace_integrity)?;
    write_project_integrity_files(workspace, &workspace_integrity)?;
    capture_workspace_layout_state(config, workspace, graph, include_dev)?;

    Ok(blocked_scripts)
}

fn run_workspace_scripts(config: &SnpmConfig, workspace: &Workspace) -> Result<Vec<String>> {
    let roots: Vec<&Path> = workspace
        .projects
        .iter()
        .map(|project| project.root.as_path())
        .collect();

    let blocked = if can_any_scripts_run(config, Some(workspace)) {
        lifecycle::run_install_scripts_for_projects(config, Some(workspace), &roots)?
    } else {
        Vec::new()
    };

    for project in &workspace.projects {
        lifecycle::run_project_scripts(config, Some(workspace), &project.root)?;
    }

    Ok(blocked)
}

fn write_project_integrity_files(
    workspace: &Workspace,
    workspace_integrity: &IntegrityState,
) -> Result<()> {
    for project in &workspace.projects {
        let project_integrity = IntegrityState {
            lockfile_hash: workspace_integrity.lockfile_hash.clone(),
            patch_hash: compute_project_patch_hash(project)?,
        };
        write_integrity_path(&project.root.join("node_modules"), &project_integrity)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run_workspace_scripts;
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};
    use crate::project::Manifest;
    use crate::{Project, Workspace};

    use std::collections::{BTreeMap, BTreeSet};
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    fn make_config() -> SnpmConfig {
        SnpmConfig {
            cache_dir: PathBuf::from("/tmp/cache"),
            data_dir: PathBuf::from("/tmp/data"),
            allow_scripts: BTreeSet::new(),
            min_package_age_days: None,
            min_package_cache_age_days: None,
            default_registry: "https://registry.npmjs.org".to_string(),
            scoped_registries: BTreeMap::new(),
            registry_auth: BTreeMap::new(),
            default_registry_auth_token: None,
            default_registry_auth_scheme: AuthScheme::Bearer,
            registry_auth_schemes: BTreeMap::new(),
            hoisting: HoistingMode::SingleVersion,
            link_backend: LinkBackend::Auto,
            strict_peers: false,
            frozen_lockfile_default: false,
            always_auth: false,
            registry_concurrency: 64,
            verbose: false,
            log_file: None,
        }
    }

    fn make_project(root: PathBuf, name: &str) -> Project {
        Project {
            manifest_path: root.join("package.json"),
            root,
            manifest: Manifest {
                name: Some(name.to_string()),
                version: Some("1.0.0".to_string()),
                private: false,
                dependencies: BTreeMap::new(),
                dev_dependencies: BTreeMap::new(),
                optional_dependencies: BTreeMap::new(),
                scripts: BTreeMap::new(),
                resolutions: BTreeMap::new(),
                files: None,
                bin: None,
                main: None,
                pnpm: None,
                snpm: None,
                workspaces: None,
            },
        }
    }

    #[test]
    fn workspace_project_scripts_run_without_dependency_allowlist() {
        let dir = tempdir().unwrap();
        let project_a_root = dir.path().join("packages").join("a");
        let project_b_root = dir.path().join("packages").join("b");

        fs::create_dir_all(&project_a_root).unwrap();
        fs::create_dir_all(&project_b_root).unwrap();
        fs::write(
            project_a_root.join("package.json"),
            r#"{
  "name": "a",
  "version": "1.0.0",
  "scripts": {
    "postinstall": "echo a > a-postinstall.txt"
  }
}
"#,
        )
        .unwrap();
        fs::write(
            project_b_root.join("package.json"),
            r#"{
  "name": "b",
  "version": "1.0.0",
  "scripts": {
    "postinstall": "echo b > b-postinstall.txt"
  }
}
"#,
        )
        .unwrap();

        let workspace = Workspace {
            root: dir.path().to_path_buf(),
            projects: vec![
                make_project(project_a_root.clone(), "a"),
                make_project(project_b_root.clone(), "b"),
            ],
            config: crate::workspace::types::WorkspaceConfig {
                packages: vec!["packages/*".to_string()],
                catalog: BTreeMap::new(),
                catalogs: BTreeMap::new(),
                only_built_dependencies: Vec::new(),
                ignored_built_dependencies: Vec::new(),
                hoisting: None,
            },
        };

        let blocked = run_workspace_scripts(&make_config(), &workspace).unwrap();

        assert!(blocked.is_empty());
        assert!(project_a_root.join("a-postinstall.txt").is_file());
        assert!(project_b_root.join("b-postinstall.txt").is_file());
    }
}

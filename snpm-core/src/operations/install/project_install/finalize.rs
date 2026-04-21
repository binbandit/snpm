use super::patches::apply_patches;
use super::plan::ProjectInstallPlan;
use crate::console;
use crate::lifecycle;
use crate::linker;
use crate::resolve::{PackageId, ResolutionGraph};
use crate::{Project, Result, SnpmConfig, Workspace};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Instant;

use crate::operations::install::utils::{
    IntegrityState, build_project_integrity_state, can_any_scripts_run,
    capture_project_layout_state, write_integrity_file,
};
use crate::operations::install::workspace::link_local_workspace_deps;

pub(super) fn finalize_install(
    config: &SnpmConfig,
    project: &Project,
    plan: &ProjectInstallPlan,
    graph: &ResolutionGraph,
    store_paths: &BTreeMap<PackageId, PathBuf>,
    include_dev: bool,
    precomputed_integrity: Option<IntegrityState>,
) -> Result<()> {
    let link_start = Instant::now();

    linker::link(
        config,
        plan.workspace.as_ref(),
        project,
        graph,
        store_paths,
        include_dev,
    )?;

    link_local_workspace_deps(
        project,
        plan.workspace.as_ref(),
        &plan.local_deps,
        &plan.local_dev_deps,
        &plan.local_optional_deps,
        include_dev,
    )?;

    let patches_applied = apply_patches(project, store_paths)?;
    if patches_applied > 0 {
        console::verbose(&format!("applied {} patches", patches_applied));
    }

    let integrity_state = match precomputed_integrity {
        Some(state) => state,
        None => build_project_integrity_state(project, graph)?,
    };
    write_integrity_file(project, &integrity_state)?;
    capture_project_layout_state(config, project, plan.workspace.as_ref(), graph, include_dev)?;

    console::verbose(&format!(
        "linking completed in {:.3}s",
        link_start.elapsed().as_secs_f64()
    ));

    Ok(())
}

pub(super) fn run_install_scripts(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    project_root: &Path,
    early_exit: bool,
) -> Result<Vec<String>> {
    if early_exit {
        console::verbose(
            "skipping dependency install scripts (early exit - node_modules is fresh)",
        );
        lifecycle::run_project_scripts(config, workspace, project_root)?;
        return Ok(Vec::new());
    }

    let blocked = if can_any_scripts_run(config, workspace) {
        lifecycle::run_install_scripts(config, workspace, project_root)?
    } else {
        console::verbose(
            "skipping dependency install scripts (no scripts can run based on config)",
        );
        Vec::new()
    };

    lifecycle::run_project_scripts(config, workspace, project_root)?;
    Ok(blocked)
}

#[cfg(test)]
mod tests {
    use super::run_install_scripts;
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};

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

    #[test]
    fn root_postinstall_runs_when_dependency_scripts_are_disallowed() {
        let dir = tempdir().unwrap();
        let project_root = dir.path();
        let dep_root = project_root.join("node_modules").join("dep");
        let root_marker = project_root.join("root-postinstall.txt");
        let dep_marker = dep_root.join("dep-postinstall.txt");

        fs::create_dir_all(&dep_root).unwrap();
        fs::write(
            project_root.join("package.json"),
            r#"{
  "name": "app",
  "version": "1.0.0",
  "scripts": {
    "postinstall": "echo root > root-postinstall.txt"
  }
}
"#,
        )
        .unwrap();
        fs::write(
            dep_root.join("package.json"),
            r#"{
  "name": "dep",
  "version": "1.0.0",
  "scripts": {
    "postinstall": "echo dep > dep-postinstall.txt"
  }
}
"#,
        )
        .unwrap();

        let blocked = run_install_scripts(&make_config(), None, project_root, false).unwrap();

        assert!(blocked.is_empty());
        assert!(root_marker.is_file());
        assert!(!dep_marker.exists());
    }

    #[test]
    fn root_postinstall_runs_on_early_exit() {
        let dir = tempdir().unwrap();
        let project_root = dir.path();
        let root_marker = project_root.join("root-postinstall.txt");

        fs::write(
            project_root.join("package.json"),
            r#"{
  "name": "app",
  "version": "1.0.0",
  "scripts": {
    "postinstall": "echo root > root-postinstall.txt"
  }
}
"#,
        )
        .unwrap();

        let blocked = run_install_scripts(&make_config(), None, project_root, true).unwrap();

        assert!(blocked.is_empty());
        assert!(root_marker.is_file());
    }
}

use crate::{SnpmConfig, Workspace};

pub fn can_any_scripts_run(config: &SnpmConfig, workspace: Option<&Workspace>) -> bool {
    if !config.allow_scripts.is_empty() {
        return true;
    }

    if let Some(workspace) = workspace {
        if !workspace.config.only_built_dependencies.is_empty() {
            return true;
        }

        if !workspace.config.ignored_built_dependencies.is_empty() {
            return true;
        }
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};
    use crate::workspace::types::{Workspace, WorkspaceConfig};
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;

    fn make_config() -> SnpmConfig {
        SnpmConfig {
            cache_dir: PathBuf::from("/tmp/cache"),
            data_dir: PathBuf::from("/tmp/data"),
            allow_scripts: BTreeSet::new(),
            disable_global_virtual_store_for_packages: BTreeSet::new(),
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
    fn can_any_scripts_run_false_by_default() {
        let config = make_config();
        assert!(!can_any_scripts_run(&config, None));
    }

    #[test]
    fn can_any_scripts_run_true_with_allow_scripts() {
        let mut config = make_config();
        config.allow_scripts.insert("esbuild".to_string());
        assert!(can_any_scripts_run(&config, None));
    }

    #[test]
    fn can_any_scripts_run_true_with_workspace_only_built() {
        let config = make_config();
        let workspace = Workspace {
            root: PathBuf::from("/workspace"),
            projects: Vec::new(),
            config: WorkspaceConfig {
                packages: Vec::new(),
                catalog: BTreeMap::new(),
                catalogs: BTreeMap::new(),
                only_built_dependencies: vec!["esbuild".to_string()],
                ignored_built_dependencies: Vec::new(),
                disable_global_virtual_store_for_packages: None,
                hoisting: None,
            },
        };
        assert!(can_any_scripts_run(&config, Some(&workspace)));
    }

    #[test]
    fn can_any_scripts_run_true_with_workspace_ignored() {
        let config = make_config();
        let workspace = Workspace {
            root: PathBuf::from("/workspace"),
            projects: Vec::new(),
            config: WorkspaceConfig {
                packages: Vec::new(),
                catalog: BTreeMap::new(),
                catalogs: BTreeMap::new(),
                only_built_dependencies: Vec::new(),
                ignored_built_dependencies: vec!["malicious".to_string()],
                disable_global_virtual_store_for_packages: None,
                hoisting: None,
            },
        };
        assert!(can_any_scripts_run(&config, Some(&workspace)));
    }
}

use crate::{SnpmConfig, Workspace};

pub(crate) fn is_dep_script_allowed(
    config: &SnpmConfig,
    workspace: Option<&Workspace>,
    name: &str,
) -> bool {
    if let Some(ws) = workspace {
        if !ws.config.only_built_dependencies.is_empty() {
            return ws
                .config
                .only_built_dependencies
                .iter()
                .any(|candidate| candidate == name);
        }

        if !ws.config.ignored_built_dependencies.is_empty() {
            return !ws
                .config
                .ignored_built_dependencies
                .iter()
                .any(|candidate| candidate == name);
        }
    }

    if !config.allow_scripts.is_empty() {
        return config.allow_scripts.contains(name);
    }

    false
}

#[cfg(test)]
mod tests {
    use super::is_dep_script_allowed;
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

    fn make_workspace(only_built: Vec<String>, ignored_built: Vec<String>) -> Workspace {
        Workspace {
            root: PathBuf::from("/workspace"),
            projects: Vec::new(),
            config: WorkspaceConfig {
                packages: Vec::new(),
                catalog: BTreeMap::new(),
                catalogs: BTreeMap::new(),
                only_built_dependencies: only_built,
                ignored_built_dependencies: ignored_built,
                disable_global_virtual_store_for_packages: None,
                hoisting: None,
            },
        }
    }

    #[test]
    fn default_blocks_all_scripts() {
        let config = make_config();
        assert!(!is_dep_script_allowed(&config, None, "esbuild"));
    }

    #[test]
    fn config_allow_scripts_permits_listed() {
        let mut config = make_config();
        config.allow_scripts.insert("esbuild".to_string());
        assert!(is_dep_script_allowed(&config, None, "esbuild"));
        assert!(!is_dep_script_allowed(&config, None, "other-pkg"));
    }

    #[test]
    fn workspace_only_built_permits_listed() {
        let config = make_config();
        let ws = make_workspace(vec!["esbuild".to_string()], vec![]);
        assert!(is_dep_script_allowed(&config, Some(&ws), "esbuild"));
        assert!(!is_dep_script_allowed(&config, Some(&ws), "other-pkg"));
    }

    #[test]
    fn workspace_ignored_built_blocks_listed() {
        let config = make_config();
        let ws = make_workspace(vec![], vec!["malicious-pkg".to_string()]);
        assert!(is_dep_script_allowed(&config, Some(&ws), "esbuild"));
        assert!(!is_dep_script_allowed(&config, Some(&ws), "malicious-pkg"));
    }

    #[test]
    fn workspace_only_built_takes_priority_over_config() {
        let mut config = make_config();
        config.allow_scripts.insert("config-allowed".to_string());
        let ws = make_workspace(vec!["ws-allowed".to_string()], vec![]);
        assert!(is_dep_script_allowed(&config, Some(&ws), "ws-allowed"));
        assert!(!is_dep_script_allowed(&config, Some(&ws), "config-allowed"));
    }

    #[test]
    fn workspace_only_built_takes_priority_over_ignored() {
        let config = make_config();
        let ws = make_workspace(vec!["allowed".to_string()], vec!["ignored".to_string()]);
        assert!(is_dep_script_allowed(&config, Some(&ws), "allowed"));
        assert!(!is_dep_script_allowed(&config, Some(&ws), "ignored"));
    }

    #[test]
    fn empty_workspace_falls_through_to_config() {
        let mut config = make_config();
        config.allow_scripts.insert("esbuild".to_string());
        let ws = make_workspace(vec![], vec![]);
        assert!(is_dep_script_allowed(&config, Some(&ws), "esbuild"));
        assert!(!is_dep_script_allowed(&config, Some(&ws), "other"));
    }
}

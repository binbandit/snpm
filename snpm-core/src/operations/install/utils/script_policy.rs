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
    use crate::config::SnpmConfig;
    use crate::workspace::types::{Workspace, WorkspaceConfig};
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    fn make_config() -> SnpmConfig {
        SnpmConfig::for_tests()
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

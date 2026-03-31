use super::super::types::RegistryConfig;
use super::apply::apply_rc_file;
use directories::BaseDirs;

use std::env;
use std::path::PathBuf;

const RC_FILES: [&str; 3] = [".snpmrc", ".npmrc", ".pnpmrc"];

pub fn read_registry_config() -> RegistryConfig {
    let mut config = RegistryConfig {
        default_registry: "https://registry.npmjs.org/".to_string(),
        ..RegistryConfig::default()
    };

    for path in home_rc_paths() {
        apply_rc_file(&path, &mut config);
    }

    for path in ancestor_rc_paths() {
        apply_rc_file(&path, &mut config);
    }

    config
}

fn home_rc_paths() -> Vec<PathBuf> {
    let Some(base_dirs) = BaseDirs::new() else {
        return Vec::new();
    };

    build_rc_paths([base_dirs.home_dir().to_path_buf()])
}

fn ancestor_rc_paths() -> Vec<PathBuf> {
    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut ancestors = Vec::new();
    let mut current = Some(cwd.clone());

    while let Some(directory) = current {
        ancestors.push(directory.clone());
        current = directory.parent().map(|path| path.to_path_buf());
        if current
            .as_ref()
            .map(|path| path == &directory)
            .unwrap_or(true)
        {
            break;
        }
    }

    ancestors.reverse();
    build_rc_paths(ancestors)
}

fn build_rc_paths(directories: impl IntoIterator<Item = PathBuf>) -> Vec<PathBuf> {
    let mut paths = Vec::new();

    for directory in directories {
        for rc_name in RC_FILES {
            paths.push(directory.join(rc_name));
        }
    }

    paths
}

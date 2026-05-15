use super::super::types::RegistryConfig;
use super::apply::apply_rc_file;
use crate::config::env_vars::{ConfigEnvPrefix, read_config_env};
use directories::BaseDirs;

use std::env;
use std::path::PathBuf;

const RC_FILES: [&str; 3] = [".snpmrc", ".npmrc", ".pnpmrc"];

pub fn read_registry_config() -> RegistryConfig {
    let mut config = RegistryConfig {
        default_registry: "https://registry.npmjs.org/".to_string(),
        ..RegistryConfig::default()
    };

    for path in user_rc_paths() {
        apply_rc_file(&path, &mut config);
    }

    for path in ancestor_rc_paths() {
        apply_rc_file(&path, &mut config);
    }

    config
}

fn user_rc_paths() -> Vec<PathBuf> {
    configured_user_rc_path()
        .map(|path| vec![path])
        .unwrap_or_else(home_rc_paths)
}

fn configured_user_rc_path() -> Option<PathBuf> {
    read_config_env(
        "npmrc_auth_file",
        &[ConfigEnvPrefix::Snpm, ConfigEnvPrefix::Pnpm],
    )
    .or_else(|| {
        read_config_env(
            "userconfig",
            &[ConfigEnvPrefix::Snpm, ConfigEnvPrefix::Pnpm],
        )
    })
    .or_else(|| read_config_env("userconfig", &[ConfigEnvPrefix::Npm]))
    .map(normalize_user_config_path)
}

fn normalize_user_config_path(path: String) -> PathBuf {
    let trimmed = path.trim();
    if let Some(path) = trimmed
        .strip_prefix("~/")
        .or_else(|| trimmed.strip_prefix("~\\"))
        && let Some(base_dirs) = BaseDirs::new()
    {
        return base_dirs.home_dir().join(path);
    }

    let path = PathBuf::from(trimmed);
    if path.is_absolute() {
        path
    } else {
        env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::ffi::OsString;
    use std::fs;
    use std::sync::{Mutex, MutexGuard, OnceLock};
    use tempfile::tempdir;

    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    struct EnvSnapshot {
        values: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvSnapshot {
        fn capture(keys: impl IntoIterator<Item = &'static str>) -> Self {
            let values: Vec<(&'static str, Option<OsString>)> = keys
                .into_iter()
                .map(|key| (key, env::var_os(key)))
                .collect();
            for (key, _) in values.iter() {
                unsafe { env::remove_var(*key) };
            }
            Self { values }
        }

        fn set(&self, key: &'static str, value: impl AsRef<std::ffi::OsStr>) {
            unsafe { env::set_var(key, value) };
        }
    }

    impl Drop for EnvSnapshot {
        fn drop(&mut self) {
            for (key, value) in self.values.iter().rev() {
                match value {
                    Some(value) => unsafe { env::set_var(key, value) },
                    None => unsafe { env::remove_var(key) },
                }
            }
        }
    }

    struct CurrentDirGuard {
        previous: PathBuf,
    }

    impl CurrentDirGuard {
        fn set(path: impl Into<PathBuf>) -> Self {
            let previous = env::current_dir().unwrap();
            env::set_current_dir(path.into()).unwrap();
            Self { previous }
        }
    }

    impl Drop for CurrentDirGuard {
        fn drop(&mut self) {
            env::set_current_dir(&self.previous).unwrap();
        }
    }

    const USERCONFIG_ENV_KEYS: [&str; 10] = [
        "snpm_config_npmrc_auth_file",
        "SNPM_CONFIG_NPMRC_AUTH_FILE",
        "snpm_config_userconfig",
        "SNPM_CONFIG_USERCONFIG",
        "pnpm_config_npmrc_auth_file",
        "PNPM_CONFIG_NPMRC_AUTH_FILE",
        "pnpm_config_userconfig",
        "PNPM_CONFIG_USERCONFIG",
        "npm_config_userconfig",
        "NPM_CONFIG_USERCONFIG",
    ];

    #[test]
    fn configured_user_rc_path_prefers_pnpm_over_npm_fallback() {
        let _lock = env_lock();
        let temp = tempdir().unwrap();
        let pnpm_path = temp.path().join("pnpm.npmrc");
        let npm_path = temp.path().join("npm.npmrc");
        let env = EnvSnapshot::capture(USERCONFIG_ENV_KEYS);
        env.set("PNPM_CONFIG_USERCONFIG", &pnpm_path);
        env.set("NPM_CONFIG_USERCONFIG", &npm_path);

        assert_eq!(
            configured_user_rc_path().as_deref(),
            Some(pnpm_path.as_path())
        );
    }

    #[test]
    fn configured_user_rc_path_ignores_empty_npm_fallback() {
        let _lock = env_lock();
        let env = EnvSnapshot::capture(USERCONFIG_ENV_KEYS);
        env.set("NPM_CONFIG_USERCONFIG", "");

        assert!(configured_user_rc_path().is_none());
    }

    #[test]
    fn read_registry_config_uses_configured_userconfig_instead_of_home_rc_paths() {
        let _lock = env_lock();
        let temp = tempdir().unwrap();
        let user_config = temp.path().join("ci.npmrc");
        fs::write(&user_config, "registry=https://ci.example.test\n").unwrap();

        let env = EnvSnapshot::capture(USERCONFIG_ENV_KEYS);
        env.set("NPM_CONFIG_USERCONFIG", &user_config);
        let _cwd = CurrentDirGuard::set(temp.path());

        let config = read_registry_config();

        assert_eq!(config.default_registry, "https://ci.example.test");
    }
}

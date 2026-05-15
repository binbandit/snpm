use super::super::rc::{host_from_url, normalize_registry_url};
use super::super::{AuthScheme, HoistingMode, LinkBackend};
use crate::config::env_vars::{ConfigEnvPrefix, read_config_env, read_non_empty_env};

use std::path::PathBuf;

pub(super) fn apply_default_registry_env(
    default_registry: &mut String,
    default_registry_auth_token: &mut Option<String>,
) {
    if let Some(new_default) = read_config_env(
        "registry",
        &[
            ConfigEnvPrefix::Snpm,
            ConfigEnvPrefix::Pnpm,
            ConfigEnvPrefix::Npm,
        ],
    ) {
        let runtime_config_host = host_from_url(default_registry);
        let new_host = host_from_url(&new_default);

        if runtime_config_host != new_host {
            *default_registry_auth_token = None;
        }

        *default_registry = normalize_registry_url(&new_default);
    }
}

pub(super) fn apply_auth_env(
    default_registry_auth_token: &mut Option<String>,
    default_registry_auth_scheme: &mut AuthScheme,
) {
    if let Some(token) = read_non_empty_env("SNPM_AUTH_TOKEN")
        .or_else(|| read_non_empty_env("NODE_AUTH_TOKEN"))
        .or_else(|| read_non_empty_env("NPM_TOKEN"))
    {
        *default_registry_auth_token = Some(token);
        *default_registry_auth_scheme = AuthScheme::Bearer;
    } else if let Some(token) = read_config_env(
        "_auth",
        &[
            ConfigEnvPrefix::Snpm,
            ConfigEnvPrefix::Pnpm,
            ConfigEnvPrefix::Npm,
        ],
    ) {
        *default_registry_auth_token = Some(token);
        *default_registry_auth_scheme = AuthScheme::Basic;
    }
}

pub(super) fn apply_install_env(
    hoisting: &mut HoistingMode,
    link_backend: &mut LinkBackend,
    strict_peers: &mut bool,
    frozen_lockfile_default: &mut bool,
    registry_concurrency: &mut usize,
    always_auth: &mut bool,
) {
    if let Some(value) = read_non_empty_env("SNPM_HOIST")
        .or_else(|| read_config_env("hoist", &[ConfigEnvPrefix::Snpm, ConfigEnvPrefix::Pnpm]))
        && let Some(mode) = HoistingMode::parse(value.trim())
    {
        *hoisting = mode;
    }

    if let Some(value) = read_non_empty_env("SNPM_LINK_BACKEND")
        .or_else(|| read_config_env("link_backend", &[ConfigEnvPrefix::Snpm]))
        && let Some(backend) = LinkBackend::parse(value.trim())
    {
        *link_backend = backend;
    }

    if let Some(value) = read_non_empty_env("SNPM_STRICT_PEERS").or_else(|| {
        read_config_env(
            "strict_peer_dependencies",
            &[ConfigEnvPrefix::Snpm, ConfigEnvPrefix::Pnpm],
        )
    }) {
        *strict_peers = env_flag_is_enabled(&value);
    }

    if let Some(value) = read_non_empty_env("SNPM_FROZEN_LOCKFILE").or_else(|| {
        read_config_env(
            "frozen_lockfile",
            &[ConfigEnvPrefix::Snpm, ConfigEnvPrefix::Pnpm],
        )
    }) {
        *frozen_lockfile_default = env_flag_is_enabled(&value);
    }

    if let Some(value) = read_non_empty_env("SNPM_REGISTRY_CONCURRENCY")
        .or_else(|| read_config_env("registry_concurrency", &[ConfigEnvPrefix::Snpm]))
        && let Ok(parsed) = value.trim().parse::<usize>()
        && parsed > 0
    {
        *registry_concurrency = parsed;
    }

    if let Some(value) = read_non_empty_env("SNPM_ALWAYS_AUTH").or_else(|| {
        read_config_env(
            "always_auth",
            &[
                ConfigEnvPrefix::Snpm,
                ConfigEnvPrefix::Pnpm,
                ConfigEnvPrefix::Npm,
            ],
        )
    }) && env_flag_is_enabled(&value)
    {
        *always_auth = true;
    }
}

pub(super) fn read_logging_env() -> (bool, Option<PathBuf>) {
    let verbose = read_non_empty_env("SNPM_VERBOSE")
        .map(|value| env_flag_is_enabled(&value))
        .unwrap_or(false);

    let log_file = read_non_empty_env("SNPM_LOG_FILE").map(PathBuf::from);

    (verbose, log_file)
}

fn env_flag_is_enabled(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "on"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::ffi::OsString;
    use std::sync::{Mutex, MutexGuard, OnceLock};

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

        fn set(&self, key: &'static str, value: &str) {
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

    #[test]
    fn default_registry_accepts_uppercase_pnpm_config() {
        let _lock = env_lock();
        let env = EnvSnapshot::capture([
            "snpm_config_registry",
            "SNPM_CONFIG_REGISTRY",
            "pnpm_config_registry",
            "PNPM_CONFIG_REGISTRY",
            "npm_config_registry",
            "NPM_CONFIG_REGISTRY",
        ]);
        env.set("PNPM_CONFIG_REGISTRY", "https://pnpm.example/");

        let mut registry = "https://registry.npmjs.org/".to_string();
        let mut token = Some("token".to_string());

        apply_default_registry_env(&mut registry, &mut token);

        assert_eq!(registry, "https://pnpm.example");
        assert!(token.is_none());
    }

    #[test]
    fn default_registry_prefers_pnpm_over_npm_config() {
        let _lock = env_lock();
        let env = EnvSnapshot::capture([
            "snpm_config_registry",
            "SNPM_CONFIG_REGISTRY",
            "pnpm_config_registry",
            "PNPM_CONFIG_REGISTRY",
            "npm_config_registry",
            "NPM_CONFIG_REGISTRY",
        ]);
        env.set("PNPM_CONFIG_REGISTRY", "https://pnpm.example/");
        env.set("NPM_CONFIG_REGISTRY", "https://npm.example/");

        let mut registry = "https://registry.npmjs.org/".to_string();
        let mut token = None;

        apply_default_registry_env(&mut registry, &mut token);

        assert_eq!(registry, "https://pnpm.example");
    }

    #[test]
    fn auth_accepts_pnpm_config_basic_auth() {
        let _lock = env_lock();
        let env = EnvSnapshot::capture([
            "SNPM_AUTH_TOKEN",
            "NODE_AUTH_TOKEN",
            "NPM_TOKEN",
            "snpm_config__auth",
            "SNPM_CONFIG__AUTH",
            "pnpm_config__auth",
            "PNPM_CONFIG__AUTH",
            "npm_config__auth",
            "NPM_CONFIG__AUTH",
        ]);
        env.set("PNPM_CONFIG__AUTH", "dXNlcjpwYXNz");

        let mut token = None;
        let mut scheme = AuthScheme::Bearer;

        apply_auth_env(&mut token, &mut scheme);

        assert_eq!(token.as_deref(), Some("dXNlcjpwYXNz"));
        assert_eq!(scheme, AuthScheme::Basic);
    }

    #[test]
    fn install_env_accepts_pnpm_config_aliases() {
        let _lock = env_lock();
        let env = EnvSnapshot::capture([
            "SNPM_HOIST",
            "SNPM_LINK_BACKEND",
            "SNPM_STRICT_PEERS",
            "SNPM_FROZEN_LOCKFILE",
            "SNPM_REGISTRY_CONCURRENCY",
            "SNPM_ALWAYS_AUTH",
            "snpm_config_hoist",
            "SNPM_CONFIG_HOIST",
            "pnpm_config_hoist",
            "PNPM_CONFIG_HOIST",
            "npm_config_hoist",
            "NPM_CONFIG_HOIST",
            "snpm_config_strict_peer_dependencies",
            "SNPM_CONFIG_STRICT_PEER_DEPENDENCIES",
            "pnpm_config_strict_peer_dependencies",
            "PNPM_CONFIG_STRICT_PEER_DEPENDENCIES",
            "npm_config_strict_peer_dependencies",
            "NPM_CONFIG_STRICT_PEER_DEPENDENCIES",
            "snpm_config_frozen_lockfile",
            "SNPM_CONFIG_FROZEN_LOCKFILE",
            "pnpm_config_frozen_lockfile",
            "PNPM_CONFIG_FROZEN_LOCKFILE",
            "npm_config_frozen_lockfile",
            "NPM_CONFIG_FROZEN_LOCKFILE",
            "snpm_config_registry_concurrency",
            "SNPM_CONFIG_REGISTRY_CONCURRENCY",
            "pnpm_config_registry_concurrency",
            "PNPM_CONFIG_REGISTRY_CONCURRENCY",
            "npm_config_registry_concurrency",
            "NPM_CONFIG_REGISTRY_CONCURRENCY",
            "snpm_config_always_auth",
            "SNPM_CONFIG_ALWAYS_AUTH",
            "pnpm_config_always_auth",
            "PNPM_CONFIG_ALWAYS_AUTH",
            "npm_config_always_auth",
            "NPM_CONFIG_ALWAYS_AUTH",
        ]);
        env.set("PNPM_CONFIG_HOIST", "none");
        env.set("PNPM_CONFIG_STRICT_PEER_DEPENDENCIES", "true");
        env.set("PNPM_CONFIG_FROZEN_LOCKFILE", "1");
        env.set("SNPM_CONFIG_REGISTRY_CONCURRENCY", "8");
        env.set("PNPM_CONFIG_ALWAYS_AUTH", "yes");

        let mut hoisting = HoistingMode::SingleVersion;
        let mut link_backend = LinkBackend::Auto;
        let mut strict_peers = false;
        let mut frozen_lockfile = false;
        let mut registry_concurrency = 128;
        let mut always_auth = false;

        apply_install_env(
            &mut hoisting,
            &mut link_backend,
            &mut strict_peers,
            &mut frozen_lockfile,
            &mut registry_concurrency,
            &mut always_auth,
        );

        assert_eq!(hoisting, HoistingMode::None);
        assert!(strict_peers);
        assert!(frozen_lockfile);
        assert_eq!(registry_concurrency, 8);
        assert!(always_auth);
    }
}

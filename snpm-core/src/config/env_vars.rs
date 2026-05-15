use std::env;

#[derive(Clone, Copy)]
pub(crate) enum ConfigEnvPrefix {
    Snpm,
    Pnpm,
    Npm,
}

impl ConfigEnvPrefix {
    fn lower(self) -> &'static str {
        match self {
            Self::Snpm => "snpm_config_",
            Self::Pnpm => "pnpm_config_",
            Self::Npm => "npm_config_",
        }
    }

    fn upper(self) -> &'static str {
        match self {
            Self::Snpm => "SNPM_CONFIG_",
            Self::Pnpm => "PNPM_CONFIG_",
            Self::Npm => "NPM_CONFIG_",
        }
    }
}

pub(crate) fn read_non_empty_env(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

pub(crate) fn read_config_env(key: &str, prefixes: &[ConfigEnvPrefix]) -> Option<String> {
    let upper_key = key.to_ascii_uppercase();

    for prefix in prefixes {
        let lower_name = format!("{}{}", prefix.lower(), key);
        if let Some(value) = read_non_empty_env(&lower_name) {
            return Some(value);
        }

        let upper_name = format!("{}{}", prefix.upper(), upper_key);
        if let Some(value) = read_non_empty_env(&upper_name) {
            return Some(value);
        }
    }

    None
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
    fn read_config_env_accepts_uppercase_pnpm_config() {
        let _lock = env_lock();
        let env = EnvSnapshot::capture(["pnpm_config_registry", "PNPM_CONFIG_REGISTRY"]);
        env.set("PNPM_CONFIG_REGISTRY", "https://pnpm.example");

        assert_eq!(
            read_config_env("registry", &[ConfigEnvPrefix::Pnpm]).as_deref(),
            Some("https://pnpm.example")
        );
    }

    #[test]
    fn read_config_env_prefers_lowercase_within_same_prefix() {
        let _lock = env_lock();
        let env = EnvSnapshot::capture(["pnpm_config_registry", "PNPM_CONFIG_REGISTRY"]);
        env.set("pnpm_config_registry", "https://lower.example");
        env.set("PNPM_CONFIG_REGISTRY", "https://upper.example");

        assert_eq!(
            read_config_env("registry", &[ConfigEnvPrefix::Pnpm]).as_deref(),
            Some("https://lower.example")
        );
    }

    #[test]
    fn read_config_env_prefers_pnpm_over_npm_when_requested() {
        let _lock = env_lock();
        let env = EnvSnapshot::capture([
            "pnpm_config_registry",
            "PNPM_CONFIG_REGISTRY",
            "npm_config_registry",
            "NPM_CONFIG_REGISTRY",
        ]);
        env.set("PNPM_CONFIG_REGISTRY", "https://pnpm.example");
        env.set("NPM_CONFIG_REGISTRY", "https://npm.example");

        assert_eq!(
            read_config_env("registry", &[ConfigEnvPrefix::Pnpm, ConfigEnvPrefix::Npm]).as_deref(),
            Some("https://pnpm.example")
        );
    }

    #[test]
    fn read_config_env_ignores_empty_values() {
        let _lock = env_lock();
        let env = EnvSnapshot::capture([
            "pnpm_config_registry",
            "PNPM_CONFIG_REGISTRY",
            "npm_config_registry",
            "NPM_CONFIG_REGISTRY",
        ]);
        env.set("PNPM_CONFIG_REGISTRY", "");
        env.set("NPM_CONFIG_REGISTRY", "https://npm.example");

        assert_eq!(
            read_config_env("registry", &[ConfigEnvPrefix::Pnpm, ConfigEnvPrefix::Npm]).as_deref(),
            Some("https://npm.example")
        );
    }

    #[test]
    fn read_config_env_builds_double_underscore_auth_key() {
        let _lock = env_lock();
        let env = EnvSnapshot::capture(["npm_config__auth", "NPM_CONFIG__AUTH"]);
        env.set("NPM_CONFIG__AUTH", "dXNlcjpwYXNz");

        assert_eq!(
            read_config_env("_auth", &[ConfigEnvPrefix::Npm]).as_deref(),
            Some("dXNlcjpwYXNz")
        );
    }
}

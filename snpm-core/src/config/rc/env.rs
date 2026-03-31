use std::collections::BTreeSet;
use std::env;

pub fn expand_env_vars(text: &str) -> String {
    let mut expanded = String::new();
    let mut index = 0;
    let bytes = text.as_bytes();

    while index < bytes.len() {
        if bytes[index] != b'$' {
            expanded.push(bytes[index] as char);
            index += 1;
            continue;
        }

        if index + 1 < bytes.len()
            && bytes[index + 1] == b'{'
            && let Some(end) = text[index + 2..].find('}')
        {
            let name = &text[index + 2..index + 2 + end];
            expanded.push_str(&env::var(name).unwrap_or_default());
            index += end + 3;
            continue;
        }

        let mut end = index + 1;
        while end < bytes.len()
            && (bytes[end] == b'_' || (bytes[end] as char).is_ascii_alphanumeric())
        {
            end += 1;
        }

        let name = &text[index + 1..end];
        if name.is_empty() {
            expanded.push('$');
            index += 1;
            continue;
        }

        expanded.push_str(&env::var(name).unwrap_or_default());
        index = end;
    }

    expanded
}

pub fn read_allow_scripts_from_env() -> BTreeSet<String> {
    let mut allowed = BTreeSet::new();

    if let Ok(value) = env::var("SNPM_ALLOW_SCRIPTS") {
        for part in value.split(',') {
            let name = part.trim();
            if !name.is_empty() {
                allowed.insert(name.to_string());
            }
        }
    }

    allowed
}

pub fn read_min_package_age_from_env() -> Option<u32> {
    parse_positive_u32_env("SNPM_MIN_PACKAGE_AGE_DAYS")
}

pub fn read_min_package_cache_age_from_env() -> Option<u32> {
    match env::var("SNPM_MIN_PACKAGE_CACHE_AGE_DAYS") {
        Ok(value) if value.trim().is_empty() => Some(7),
        Ok(_) => parse_positive_u32_env("SNPM_MIN_PACKAGE_CACHE_AGE_DAYS").or(Some(7)),
        Err(_) => Some(7),
    }
}

fn parse_positive_u32_env(key: &str) -> Option<u32> {
    let value = env::var(key).ok()?;
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    let parsed = trimmed.parse::<u32>().ok()?;
    (parsed > 0).then_some(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::sync::{Mutex, MutexGuard, OnceLock};

    fn env_lock() -> MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    struct EnvVarGuard {
        key: &'static str,
        previous: Option<OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: Option<&str>) -> Self {
            let previous = env::var_os(key);

            match value {
                Some(value) => unsafe { env::set_var(key, value) },
                None => unsafe { env::remove_var(key) },
            }

            Self { key, previous }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match &self.previous {
                Some(value) => unsafe { env::set_var(self.key, value) },
                None => unsafe { env::remove_var(self.key) },
            }
        }
    }

    #[test]
    fn expand_env_vars_dollar_brace_syntax() {
        let _lock = env_lock();
        let _guard = EnvVarGuard::set("SNPM_TEST_VAR_1", Some("hello"));
        assert_eq!(expand_env_vars("${SNPM_TEST_VAR_1}_world"), "hello_world");
    }

    #[test]
    fn expand_env_vars_dollar_syntax() {
        let _lock = env_lock();
        let _guard = EnvVarGuard::set("SNPM_TEST_VAR_2", Some("value"));
        assert_eq!(expand_env_vars("prefix_$SNPM_TEST_VAR_2"), "prefix_value");
    }

    #[test]
    fn expand_env_vars_missing_var() {
        let _lock = env_lock();
        let _guard = EnvVarGuard::set("SNPM_NONEXISTENT_VAR", None);
        assert_eq!(expand_env_vars("$SNPM_NONEXISTENT_VAR"), "");
    }

    #[test]
    fn expand_env_vars_no_vars() {
        assert_eq!(expand_env_vars("no vars here"), "no vars here");
    }

    #[test]
    fn expand_env_vars_bare_dollar() {
        assert_eq!(expand_env_vars("cost is $"), "cost is $");
    }
}

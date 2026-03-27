use super::{AuthScheme, HoistingMode};
use directories::BaseDirs;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::{env, fs, path::Path};

pub fn expand_env_vars(text: &str) -> String {
    use std::env;

    let mut out = String::new();
    let mut i = 0;
    let bytes = text.as_bytes();

    while i < bytes.len() {
        if bytes[i] == b'$' {
            if i + 1 < bytes.len()
                && bytes[i + 1] == b'{'
                && let Some(end) = text[i + 2..].find('}')
            {
                let var = &text[i + 2..i + 2 + end];
                let val = env::var(var).unwrap_or_default();
                out.push_str(&val);
                i += 2 + end + 1;
                continue;
            }

            let mut j = i + 1;
            while j < bytes.len()
                && (bytes[j] == b'_' || (bytes[j] as char).is_ascii_alphanumeric())
            {
                j += 1;
            }

            let var = &text[i + 1..j];
            if !var.is_empty() {
                let val = env::var(var).unwrap_or_default();
                out.push_str(&val);
                i = j;
                continue;
            }

            out.push('$');
            i += 1;
            continue;
        } else {
            out.push(bytes[i] as char);
            i += 1;
        }
    }

    out
}

pub fn normalize_registry_url(value: &str) -> String {
    let mut url = if value.starts_with("//") {
        format!("https:{}", value)
    } else {
        value.to_string()
    };

    if url.ends_with('/') {
        url.pop();
    }

    let (scheme, rest) = if let Some(r) = url.strip_prefix("https://") {
        ("https", r)
    } else if let Some(r) = url.strip_prefix("http://") {
        ("http", r)
    } else if let Some(r) = url.strip_prefix("//") {
        ("https", r)
    } else {
        return url;
    };

    let mut parts = rest.splitn(2, '/');
    let hostport = parts.next().unwrap_or("").to_ascii_lowercase();
    let suffix = parts.next().unwrap_or("");

    let mut host = hostport.clone();
    if let Some((host_part, port_part)) = hostport.split_once(':') {
        let default_https = scheme == "https" && port_part == "443";
        let default_http = scheme == "http" && port_part == "80";
        if default_https || default_http {
            host = host_part.to_string();
        }
    }

    if suffix.is_empty() {
        format!("{}://{}", scheme, host)
    } else {
        format!("{}://{}/{}", scheme, host, suffix)
    }
}

pub fn read_allow_scripts_from_env() -> BTreeSet<String> {
    let mut set = BTreeSet::new();

    if let Ok(value) = env::var("SNPM_ALLOW_SCRIPTS") {
        for part in value.split(',') {
            let name = part.trim();
            if !name.is_empty() {
                set.insert(name.to_string());
            }
        }
    }

    set
}

pub fn read_min_package_age_from_env() -> Option<u32> {
    if let Ok(value) = env::var("SNPM_MIN_PACKAGE_AGE_DAYS") {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }

        if let Ok(parsed) = trimmed.parse::<u32>()
            && parsed > 0
        {
            return Some(parsed);
        }
    }

    None
}

pub fn read_min_package_cache_age_from_env() -> Option<u32> {
    if let Ok(value) = env::var("SNPM_MIN_PACKAGE_CACHE_AGE_DAYS") {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return Some(7);
        }

        if let Ok(parsed) = trimmed.parse::<u32>()
            && parsed > 0
        {
            return Some(parsed);
        }
    }

    Some(7)
}

#[derive(Default)]
pub struct RegistryConfig {
    pub default_registry: String,
    pub scoped: BTreeMap<String, String>,
    pub registry_auth: BTreeMap<String, String>,
    pub registry_auth_schemes: BTreeMap<String, AuthScheme>,
    pub default_auth_token: Option<String>,
    pub hoisting: Option<HoistingMode>,
    pub default_auth_basic: bool,
    pub always_auth: bool,
}

pub fn read_registry_config() -> RegistryConfig {
    let mut config = RegistryConfig {
        default_registry: "https://registry.npmjs.org/".to_string(),
        ..RegistryConfig::default()
    };

    if let Some(base) = BaseDirs::new() {
        let home = base.home_dir();
        let rc_files = [".snpmrc", ".npmrc", ".pnpmrc"];

        for rc_name in rc_files.iter() {
            let path = home.join(rc_name);
            apply_rc_file(&path, &mut config);
        }
    }

    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let rc_files = [".snpmrc", ".npmrc", ".pnpmrc"];

    // Collect ancestor directories from CWD to root, then apply in reverse
    // (root first, CWD last) so that closest-to-project rc files take precedence
    let mut ancestors = Vec::new();
    let mut directory = Some(cwd.clone());
    while let Some(dir) = directory {
        ancestors.push(dir.clone());
        directory = dir.parent().map(|p| p.to_path_buf());
        if directory.as_ref().map(|d| d == &dir).unwrap_or(true) {
            break;
        }
    }

    for dir in ancestors.iter().rev() {
        for rc_name in rc_files.iter() {
            let path = dir.join(rc_name);
            apply_rc_file(&path, &mut config);
        }
    }

    config
}

pub fn apply_rc_file(path: &Path, config: &mut RegistryConfig) {
    if !path.is_file() {
        return;
    }

    if let Ok(data) = fs::read_to_string(path) {
        for line in data.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
                continue;
            }

            if let Some(equals_index) = trimmed.find('=') {
                let (key, value) = trimmed.split_at(equals_index);
                let key = key.trim();
                let mut value = expand_env_vars(value[1..].trim());

                if value.ends_with('/') && !key.starts_with("//") {
                    value.pop();
                }

                if key == "registry" {
                    if !value.is_empty() {
                        config.default_registry = normalize_registry_url(&value);
                    }
                } else if key == "snpm-hoist" || key == "snpm.hoist" || key == "snpm_hoist" {
                    if let Some(mode) = HoistingMode::parse(&value) {
                        config.hoisting = Some(mode);
                    }
                } else if let Some(scope) = key.strip_suffix(":registry") {
                    let scope = scope.trim();
                    if !scope.is_empty() && !value.is_empty() {
                        config
                            .scoped
                            .insert(scope.to_string(), normalize_registry_url(&value));
                    }
                } else if let Some(rest) = key.strip_prefix("//") {
                    let (host_and_path, auth_scheme) =
                        if let Some(prefix) = rest.strip_suffix("/:_authToken") {
                            (prefix, Some(AuthScheme::Bearer))
                        } else if let Some(prefix) = rest.strip_suffix(":_authToken") {
                            (prefix, Some(AuthScheme::Bearer))
                        } else if let Some(prefix) = rest.strip_suffix("/:_auth") {
                            (prefix, Some(AuthScheme::Basic))
                        } else if let Some(prefix) = rest.strip_suffix(":_auth") {
                            (prefix, Some(AuthScheme::Basic))
                        } else {
                            ("", None)
                        };

                    if !host_and_path.is_empty() {
                        let raw_host = host_and_path
                            .split('/')
                            .next()
                            .unwrap_or("")
                            .trim()
                            .trim_end_matches('/');

                        // Normalize host:
                        // - Lowercase
                        // - Strip default ports (:443, :80) to match PNPM behavior
                        let mut host = raw_host.to_ascii_lowercase();
                        if let Some((split_host, split_port)) = host.split_once(':')
                            && (split_port == "443" || split_port == "80")
                        {
                            host = split_host.to_string();
                        }

                        if !host.is_empty() && !value.is_empty() {
                            config.registry_auth.insert(host.to_string(), value);
                            if let Some(scheme) = auth_scheme {
                                config
                                    .registry_auth_schemes
                                    .insert(host.to_string(), scheme);
                            }
                        }
                    }
                } else if key == "_authToken" {
                    if !value.is_empty() {
                        config.default_auth_token = Some(value);
                    }
                } else if key == "_auth" {
                    // Support legacy _auth entries (basic auth). Store as default token and mark scheme
                    if !value.is_empty() {
                        config.default_auth_token = Some(value);
                        config.default_auth_basic = true;
                    }
                } else if key == "always-auth" || key == "always_auth" || key == "always.auth" {
                    let value = value.trim().to_ascii_lowercase();
                    config.always_auth =
                        matches!(value.as_str(), "1" | "true" | "yes" | "y" | "on");
                }
            }
        }
    }
}

pub(crate) fn host_from_url(url: &str) -> Option<String> {
    let trimmed = url.trim();
    if trimmed.is_empty() {
        return None;
    }

    let is_https = trimmed.starts_with("https://");
    let is_http = trimmed.starts_with("http://");

    let without_scheme = trimmed
        .strip_prefix("https://")
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);

    let hostport = without_scheme.split('/').next().unwrap_or("").trim();

    if hostport.is_empty() {
        return None;
    }

    let mut host = hostport.to_ascii_lowercase();

    if let Some((split_host, split_port)) = host.split_once(':') {
        let default_https = is_https && split_port == "443";
        let default_http = is_http && split_port == "80";
        if default_https || default_http {
            host = split_host.to_string();
        }
    }

    if host.is_empty() { None } else { Some(host) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn parses_scoped_basic_auth_entries() {
        let file = NamedTempFile::new().unwrap();
        fs::write(
            file.path(),
            "//registry.example.com/:_auth=dGVzdDp0b2tlbg==\n",
        )
        .unwrap();

        let mut config = RegistryConfig::default();

        apply_rc_file(file.path(), &mut config);

        assert_eq!(
            config
                .registry_auth
                .get("registry.example.com")
                .map(String::as_str),
            Some("dGVzdDp0b2tlbg==")
        );
        assert_eq!(
            config.registry_auth_schemes.get("registry.example.com"),
            Some(&AuthScheme::Basic)
        );
        assert!(config.default_auth_token.is_none());
        assert!(!config.default_auth_basic);
    }

    #[test]
    fn normalize_registry_url_strips_trailing_slash() {
        assert_eq!(
            normalize_registry_url("https://registry.npmjs.org/"),
            "https://registry.npmjs.org"
        );
    }

    #[test]
    fn normalize_registry_url_lowercases_host() {
        assert_eq!(
            normalize_registry_url("https://Registry.NPMjs.ORG"),
            "https://registry.npmjs.org"
        );
    }

    #[test]
    fn normalize_registry_url_strips_default_https_port() {
        assert_eq!(
            normalize_registry_url("https://registry.npmjs.org:443/"),
            "https://registry.npmjs.org"
        );
    }

    #[test]
    fn normalize_registry_url_strips_default_http_port() {
        assert_eq!(
            normalize_registry_url("http://localhost:80/"),
            "http://localhost"
        );
    }

    #[test]
    fn normalize_registry_url_keeps_custom_port() {
        assert_eq!(
            normalize_registry_url("http://localhost:4873"),
            "http://localhost:4873"
        );
    }

    #[test]
    fn normalize_registry_url_adds_https_to_double_slash() {
        assert_eq!(
            normalize_registry_url("//registry.npmjs.org/"),
            "https://registry.npmjs.org"
        );
    }

    #[test]
    fn normalize_registry_url_preserves_path() {
        assert_eq!(
            normalize_registry_url("https://npm.pkg.github.com/owner"),
            "https://npm.pkg.github.com/owner"
        );
    }

    #[test]
    fn host_from_url_basic() {
        assert_eq!(
            host_from_url("https://registry.npmjs.org/foo"),
            Some("registry.npmjs.org".to_string())
        );
    }

    #[test]
    fn host_from_url_strips_default_port() {
        assert_eq!(
            host_from_url("https://registry.npmjs.org:443/foo"),
            Some("registry.npmjs.org".to_string())
        );
    }

    #[test]
    fn host_from_url_keeps_custom_port() {
        assert_eq!(
            host_from_url("http://localhost:4873"),
            Some("localhost:4873".to_string())
        );
    }

    #[test]
    fn host_from_url_empty_returns_none() {
        assert_eq!(host_from_url(""), None);
    }

    #[test]
    fn host_from_url_lowercases() {
        assert_eq!(
            host_from_url("https://Registry.NPMjs.ORG/"),
            Some("registry.npmjs.org".to_string())
        );
    }

    #[test]
    fn expand_env_vars_dollar_brace_syntax() {
        unsafe { std::env::set_var("SNPM_TEST_VAR_1", "hello") };
        assert_eq!(expand_env_vars("${SNPM_TEST_VAR_1}_world"), "hello_world");
        unsafe { std::env::remove_var("SNPM_TEST_VAR_1") };
    }

    #[test]
    fn expand_env_vars_dollar_syntax() {
        unsafe { std::env::set_var("SNPM_TEST_VAR_2", "value") };
        assert_eq!(expand_env_vars("prefix_$SNPM_TEST_VAR_2"), "prefix_value");
        unsafe { std::env::remove_var("SNPM_TEST_VAR_2") };
    }

    #[test]
    fn expand_env_vars_missing_var() {
        unsafe { std::env::remove_var("SNPM_NONEXISTENT_VAR") };
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

    #[test]
    fn apply_rc_file_parses_registry() {
        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), "registry=https://custom.registry.com/\n").unwrap();

        let mut config = RegistryConfig {
            default_registry: "https://registry.npmjs.org/".to_string(),
            ..RegistryConfig::default()
        };
        apply_rc_file(file.path(), &mut config);

        assert_eq!(config.default_registry, "https://custom.registry.com");
    }

    #[test]
    fn apply_rc_file_parses_scoped_registry() {
        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), "@myorg:registry=https://npm.myorg.com/\n").unwrap();

        let mut config = RegistryConfig::default();
        apply_rc_file(file.path(), &mut config);

        assert_eq!(
            config.scoped.get("@myorg").map(String::as_str),
            Some("https://npm.myorg.com")
        );
    }

    #[test]
    fn apply_rc_file_parses_auth_token() {
        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), "//registry.example.com/:_authToken=my-token\n").unwrap();

        let mut config = RegistryConfig::default();
        apply_rc_file(file.path(), &mut config);

        assert_eq!(
            config
                .registry_auth
                .get("registry.example.com")
                .map(String::as_str),
            Some("my-token")
        );
        assert_eq!(
            config.registry_auth_schemes.get("registry.example.com"),
            Some(&AuthScheme::Bearer)
        );
    }

    #[test]
    fn apply_rc_file_parses_default_auth_token() {
        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), "_authToken=default-token\n").unwrap();

        let mut config = RegistryConfig::default();
        apply_rc_file(file.path(), &mut config);

        assert_eq!(config.default_auth_token.as_deref(), Some("default-token"));
    }

    #[test]
    fn apply_rc_file_parses_legacy_auth() {
        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), "_auth=dXNlcjpwYXNz\n").unwrap();

        let mut config = RegistryConfig::default();
        apply_rc_file(file.path(), &mut config);

        assert_eq!(config.default_auth_token.as_deref(), Some("dXNlcjpwYXNz"));
        assert!(config.default_auth_basic);
    }

    #[test]
    fn apply_rc_file_skips_comments() {
        let file = NamedTempFile::new().unwrap();
        fs::write(
            file.path(),
            "# This is a comment\n; also a comment\nregistry=https://custom.com\n",
        )
        .unwrap();

        let mut config = RegistryConfig {
            default_registry: "https://registry.npmjs.org/".to_string(),
            ..RegistryConfig::default()
        };
        apply_rc_file(file.path(), &mut config);

        assert_eq!(config.default_registry, "https://custom.com");
    }

    #[test]
    fn apply_rc_file_parses_hoisting() {
        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), "snpm-hoist=none\n").unwrap();

        let mut config = RegistryConfig::default();
        apply_rc_file(file.path(), &mut config);

        assert_eq!(config.hoisting, Some(HoistingMode::None));
    }

    #[test]
    fn apply_rc_file_parses_always_auth() {
        let file = NamedTempFile::new().unwrap();
        fs::write(file.path(), "always-auth=true\n").unwrap();

        let mut config = RegistryConfig::default();
        apply_rc_file(file.path(), &mut config);

        assert!(config.always_auth);
    }

    #[test]
    fn apply_rc_file_nonexistent_is_noop() {
        let mut config = RegistryConfig::default();
        apply_rc_file(Path::new("/nonexistent/.snpmrc"), &mut config);
        assert_eq!(config.default_auth_token, None);
    }
}

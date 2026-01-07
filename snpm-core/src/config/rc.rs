use super::HoistingMode;
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

pub fn read_registry_config() -> (
    String,
    BTreeMap<String, String>,
    BTreeMap<String, String>,
    Option<String>,
    Option<HoistingMode>,
    bool,
) {
    let mut default_registry = "https://registry.npmjs.org/".to_string();
    let mut scoped = BTreeMap::new();
    let mut registry_auth = BTreeMap::new();
    let mut default_auth_token = None;
    let mut hoisting = None;
    let mut rc_default_auth_basic = false;

    if let Some(base) = BaseDirs::new() {
        let home = base.home_dir();
        let rc_files = [".snpmrc", ".npmrc", ".pnpmrc"];

        for rc_name in rc_files.iter() {
            let path = home.join(rc_name);
            apply_rc_file(
                &path,
                &mut default_registry,
                &mut scoped,
                &mut registry_auth,
                &mut default_auth_token,
                &mut hoisting,
                &mut rc_default_auth_basic,
            );
        }
    }

    let cwd = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let rc_files = [".snpmrc", ".npmrc", ".pnpmrc"];

    // Walk up parent directories to root to respect rc precedence across nested workspaces
    let mut directory = Some(cwd.clone());
    while let Some(dir) = directory {
        for rc_name in rc_files.iter() {
            let path = dir.join(rc_name);

            apply_rc_file(
                &path,
                &mut default_registry,
                &mut scoped,
                &mut registry_auth,
                &mut default_auth_token,
                &mut hoisting,
                &mut rc_default_auth_basic,
            );
        }

        directory = dir.parent().map(|p| p.to_path_buf());
        if directory.as_ref().map(|d| d == &dir).unwrap_or(true) {
            break;
        }
    }

    (
        default_registry,
        scoped,
        registry_auth,
        default_auth_token,
        hoisting,
        rc_default_auth_basic,
    )
}

pub fn apply_rc_file(
    path: &Path,
    default_registry: &mut String,
    scoped: &mut BTreeMap<String, String>,
    registry_auth: &mut BTreeMap<String, String>,
    default_auth_token: &mut Option<String>,
    hoisting: &mut Option<HoistingMode>,
    default_auth_basic: &mut bool,
) {
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
                        *default_registry = normalize_registry_url(&value);
                    }
                } else if key == "snpm-hoist" || key == "snpm.hoist" || key == "snpm_hoist" {
                    if let Some(mode) = HoistingMode::from_str(&value) {
                        *hoisting = Some(mode);
                    }
                } else if let Some(scope) = key.strip_suffix(":registry") {
                    let scope = scope.trim();
                    if !scope.is_empty() && !value.is_empty() {
                        scoped.insert(scope.to_string(), normalize_registry_url(&value));
                    }
                } else if let Some(rest) = key.strip_prefix("//") {
                    let host_and_path = if let Some(prefix) = rest.strip_suffix("/:_authToken") {
                        prefix
                    } else if let Some(prefix) = rest.strip_suffix(":_authToken") {
                        prefix
                    } else {
                        ""
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
                            registry_auth.insert(host.to_string(), value);
                        }
                    }
                } else if key == "_authToken" {
                    if !value.is_empty() {
                        *default_auth_token = Some(value);
                    }
                } else if key == "_auth" {
                    // Support legacy _auth entries (basic auth). Store as default token and mark scheme
                    if !value.is_empty() {
                        *default_auth_token = Some(value);
                        *default_auth_basic = true;
                    }
                } else if key == "always-auth" || key == "always_auth" || key == "always.auth" {
                    let value = value.trim().to_ascii_lowercase();
                    let on = matches!(value.as_str(), "1" | "true" | "yes" | "y" | "on");
                    if on {
                        // This flag is applied in from_env() defaults
                        // Kept here for symmetry with pnpm configs
                    }
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

use super::super::super::types::RegistryConfig;
use crate::config::AuthScheme;

pub(super) fn apply_scoped_auth(config: &mut RegistryConfig, rest: &str, value: &str) {
    let Some((host_and_path, auth_scheme)) = parse_scoped_auth_key(rest) else {
        return;
    };

    if value.is_empty() {
        return;
    }

    let host = normalize_auth_host(host_and_path);
    if host.is_empty() {
        return;
    }

    config.registry_auth.insert(host.clone(), value.to_string());
    config.registry_auth_schemes.insert(host, auth_scheme);
}

fn parse_scoped_auth_key(rest: &str) -> Option<(&str, AuthScheme)> {
    if let Some(prefix) = rest.strip_suffix("/:_authToken") {
        return Some((prefix, AuthScheme::Bearer));
    }
    if let Some(prefix) = rest.strip_suffix(":_authToken") {
        return Some((prefix, AuthScheme::Bearer));
    }
    if let Some(prefix) = rest.strip_suffix("/:_auth") {
        return Some((prefix, AuthScheme::Basic));
    }
    if let Some(prefix) = rest.strip_suffix(":_auth") {
        return Some((prefix, AuthScheme::Basic));
    }

    None
}

fn normalize_auth_host(host_and_path: &str) -> String {
    let raw_host = host_and_path
        .split('/')
        .next()
        .unwrap_or("")
        .trim()
        .trim_end_matches('/');

    let host = raw_host.to_ascii_lowercase();
    let Some((name, port)) = host.split_once(':') else {
        return host;
    };

    if port == "80" || port == "443" {
        name.to_string()
    } else {
        host
    }
}

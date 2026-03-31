use super::super::rc::{host_from_url, normalize_registry_url};
use super::super::{AuthScheme, HoistingMode, LinkBackend};

use std::env;
use std::path::PathBuf;

pub(super) fn apply_default_registry_env(
    default_registry: &mut String,
    default_registry_auth_token: &mut Option<String>,
) {
    if let Ok(value) = env::var("NPM_CONFIG_REGISTRY").or_else(|_| env::var("npm_config_registry"))
    {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            let new_default = trimmed.to_string();
            let runtime_config_host = host_from_url(default_registry);
            let new_host = host_from_url(&new_default);

            if runtime_config_host != new_host {
                *default_registry_auth_token = None;
            }

            *default_registry = normalize_registry_url(&new_default);
        }
    }
}

pub(super) fn apply_auth_env(
    default_registry_auth_token: &mut Option<String>,
    default_registry_auth_scheme: &mut AuthScheme,
) {
    if let Ok(token) = env::var("NODE_AUTH_TOKEN")
        .or_else(|_| env::var("NPM_TOKEN"))
        .or_else(|_| env::var("SNPM_AUTH_TOKEN"))
    {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            *default_registry_auth_token = Some(trimmed.to_string());
            *default_registry_auth_scheme = AuthScheme::Bearer;
        }
    } else if let Ok(token) = env::var("NPM_CONFIG__AUTH").or_else(|_| env::var("npm_config__auth"))
    {
        let trimmed = token.trim();
        if !trimmed.is_empty() {
            *default_registry_auth_token = Some(trimmed.to_string());
            *default_registry_auth_scheme = AuthScheme::Basic;
        }
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
    if let Ok(value) = env::var("SNPM_HOIST")
        && let Some(mode) = HoistingMode::parse(value.trim())
    {
        *hoisting = mode;
    }

    if let Ok(value) = env::var("SNPM_LINK_BACKEND")
        && let Some(backend) = LinkBackend::parse(value.trim())
    {
        *link_backend = backend;
    }

    if let Ok(value) = env::var("SNPM_STRICT_PEERS") {
        *strict_peers = env_flag_is_enabled(&value);
    }

    if let Ok(value) = env::var("SNPM_FROZEN_LOCKFILE") {
        *frozen_lockfile_default = env_flag_is_enabled(&value);
    }

    if let Ok(value) = env::var("SNPM_REGISTRY_CONCURRENCY")
        && let Ok(parsed) = value.trim().parse::<usize>()
        && parsed > 0
    {
        *registry_concurrency = parsed;
    }

    if let Ok(value) = env::var("NPM_CONFIG_ALWAYS_AUTH")
        .or_else(|_| env::var("npm_config_always_auth"))
        .or_else(|_| env::var("SNPM_ALWAYS_AUTH"))
        && env_flag_is_enabled(&value)
    {
        *always_auth = true;
    }
}

pub(super) fn read_logging_env() -> (bool, Option<PathBuf>) {
    let verbose = env::var("SNPM_VERBOSE")
        .map(|value| env_flag_is_enabled(&value))
        .unwrap_or(false);

    let log_file = env::var("SNPM_LOG_FILE")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .map(PathBuf::from);

    (verbose, log_file)
}

fn env_flag_is_enabled(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "on"
    )
}

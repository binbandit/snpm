mod matching;
mod rc_file;
mod scope;

use crate::{Result, SnpmConfig, SnpmError};

use matching::should_keep_line;
use rc_file::{rc_path, read_rc_file, write_rc_file};
use scope::validate_scope;

pub fn save_credentials(
    config: &SnpmConfig,
    registry: Option<&str>,
    token: &str,
    scope: Option<&str>,
) -> Result<()> {
    let token = token.trim();
    if token.is_empty() {
        return Err(SnpmError::Auth {
            reason: "token cannot be empty".into(),
        });
    }

    let registry_url = registry
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(config.default_registry.as_str());
    let host = registry_host(registry_url)?;
    let validated_scope = validate_scope(scope)?;
    let rc_path = rc_path();

    let mut lines: Vec<String> = read_rc_file(&rc_path)?
        .into_iter()
        .filter(|line| should_keep_line(line, &host, validated_scope.as_deref()))
        .collect();

    lines.push(format!("//{host}/:_authToken={token}"));

    if let Some(scope_name) = &validated_scope {
        lines.push(format!("{scope_name}:registry={registry_url}"));
    }

    write_rc_file(&rc_path, &lines)
}

pub fn login(
    config: &SnpmConfig,
    registry: Option<&str>,
    token: &str,
    scope: Option<&str>,
) -> Result<()> {
    save_credentials(config, registry, token, scope)
}

pub fn logout(config: &SnpmConfig, registry: Option<&str>, scope: Option<&str>) -> Result<()> {
    let registry_url = registry
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(config.default_registry.as_str());
    let host = registry_host(registry_url)?;
    let validated_scope = validate_scope(scope)?;
    let rc_path = rc_path();

    if !rc_path.is_file() {
        return Ok(());
    }

    let original_lines = read_rc_file(&rc_path)?;
    let filtered_lines: Vec<String> = original_lines
        .iter()
        .filter(|line| should_keep_line(line, &host, validated_scope.as_deref()))
        .cloned()
        .collect();

    if filtered_lines.len() == original_lines.len() {
        return Err(SnpmError::Auth {
            reason: format!("no credentials found for {registry_url}"),
        });
    }

    write_rc_file(&rc_path, &filtered_lines)
}

fn registry_host(registry_url: &str) -> Result<String> {
    crate::config::host_from_url(registry_url).ok_or_else(|| SnpmError::Auth {
        reason: format!("invalid registry URL: {registry_url}"),
    })
}

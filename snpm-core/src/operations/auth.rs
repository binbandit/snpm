use crate::{Result, SnpmConfig, SnpmError};
use directories::BaseDirs;
use std::fs;
use std::path::PathBuf;

pub fn login(
    config: &SnpmConfig,
    registry: Option<&str>,
    token: &str,
    scope: Option<&str>,
) -> Result<()> {
    let token = token.trim();
    if token.is_empty() {
        return Err(SnpmError::Auth {
            reason: "auth token is empty".into(),
        });
    }

    let registry_url = registry
        .and_then(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .unwrap_or(config.default_registry.as_str());

    let host = crate::config::host_from_url(registry_url).ok_or_else(|| SnpmError::Auth {
        reason: format!("invalid registry URL {registry_url}"),
    })?;

    let scope = validate_scope(scope)?;
    let rc_path = rc_path();
    let existing_lines = read_rc_file(&rc_path)?;
    let filtered_lines = filter_existing_credentials(&existing_lines, &host, scope.as_deref());

    let mut lines = filtered_lines;
    lines.push(format!("//{host}/:_authToken={token}"));

    if let Some(scope_name) = scope {
        lines.push(format!("{scope_name}:registry={registry_url}"));
    }

    write_rc_file(&rc_path, &lines)?;

    Ok(())
}

fn validate_scope(scope: Option<&str>) -> Result<Option<String>> {
    let Some(s) = scope else {
        return Ok(None);
    };

    let trimmed = s.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    if !trimmed.starts_with('@') {
        return Err(SnpmError::Auth {
            reason: format!("scope must start with @ (got: {trimmed})"),
        });
    }

    if trimmed.len() == 1 {
        return Err(SnpmError::Auth {
            reason: "scope cannot be just @".into(),
        });
    }

    Ok(Some(trimmed.to_string()))
}

fn rc_path() -> PathBuf {
    BaseDirs::new()
        .map(|base| base.home_dir().join(".snpmrc"))
        .unwrap_or_else(|| PathBuf::from(".snpmrc"))
}

fn read_rc_file(path: &PathBuf) -> Result<Vec<String>> {
    if !path.is_file() {
        return Ok(Vec::new());
    }

    let data = fs::read_to_string(path).map_err(|source| SnpmError::ReadFile {
        path: path.clone(),
        source,
    })?;

    Ok(data.lines().map(|s| s.to_string()).collect())
}

fn write_rc_file(path: &PathBuf, lines: &[String]) -> Result<()> {
    if lines.is_empty() {
        return fs::write(path, "").map_err(|source| SnpmError::WriteFile {
            path: path.clone(),
            source,
        });
    }

    let mut contents = lines.join("\n");
    contents.push('\n');

    fs::write(path, contents).map_err(|source| SnpmError::WriteFile {
        path: path.clone(),
        source,
    })
}

fn filter_existing_credentials(lines: &[String], host: &str, scope: Option<&str>) -> Vec<String> {
    lines
        .iter()
        .filter_map(|line| {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                return None;
            }

            if is_auth_line_for_host(trimmed, host) {
                return None;
            }

            if let Some(scope_name) = scope
                && is_scope_registry_line(trimmed, scope_name) {
                    return None;
                }

            Some(line.clone())
        })
        .collect()
}

fn is_auth_line_for_host(line: &str, host: &str) -> bool {
    if !line.starts_with("//") {
        return false;
    }

    let Some(eq_idx) = line.find('=') else {
        return false;
    };

    let key = &line[..eq_idx];
    let Some(rest) = key.strip_prefix("//") else {
        return false;
    };

    let host_part = rest
        .strip_suffix("/:_authToken")
        .or_else(|| rest.strip_suffix(":_authToken"))
        .unwrap_or("");

    let extracted_host = host_part
        .split('/')
        .next()
        .unwrap_or("")
        .trim_end_matches('/');

    extracted_host == host
}

fn is_scope_registry_line(line: &str, scope: &str) -> bool {
    if !line.starts_with('@') {
        return false;
    }

    let Some(eq_idx) = line.find('=') else {
        return false;
    };

    let key = &line[..eq_idx];
    let Some(extracted_scope) = key.strip_suffix(":registry") else {
        return false;
    };

    extracted_scope.trim() == scope
}

pub fn logout(config: &SnpmConfig, registry: Option<&str>, scope: Option<&str>) -> Result<()> {
    let registry_url = registry
        .and_then(|s| {
            let trimmed = s.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
        .unwrap_or(config.default_registry.as_str());

    let host = crate::config::host_from_url(registry_url).ok_or_else(|| SnpmError::Auth {
        reason: format!("invalid registry URL {registry_url}"),
    })?;

    let scope = validate_scope(scope)?;
    let rc_path = rc_path();

    if !rc_path.is_file() {
        return Ok(());
    }

    let existing_lines = read_rc_file(&rc_path)?;
    let original_count = existing_lines.len();
    let filtered_lines = filter_existing_credentials(&existing_lines, &host, scope.as_deref());

    if filtered_lines.len() == original_count {
        return Err(SnpmError::Auth {
            reason: format!("no credentials found for {registry_url}"),
        });
    }

    write_rc_file(&rc_path, &filtered_lines)?;

    Ok(())
}

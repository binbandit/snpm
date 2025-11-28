use crate::{Result, SnpmConfig, SnpmError};
use directories::BaseDirs;
use std::fs;
use std::path::PathBuf;

pub fn login(config: &SnpmConfig, registry: Option<&str>, token: &str) -> Result<()> {
    let trimmed_token = token.trim();

    if trimmed_token.is_empty() {
        return Err(SnpmError::Auth {
            reason: "auth token is empty".into(),
        });
    }

    let registry_url = registry
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| config.default_registry.as_str());

    let host = crate::config::host_from_url(registry_url).ok_or_else(|| SnpmError::Auth {
        reason: format!("invalid registry URL {registry_url}"),
    })?;

    let rc_path = default_rc_path();

    let mut lines = Vec::new();

    if rc_path.is_file() {
        let data = fs::read_to_string(&rc_path).map_err(|source| SnpmError::ReadFile {
            path: rc_path.clone(),
            source,
        })?;

        for line in data.lines() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                lines.push(line.to_string());
                continue;
            }

            if let Some(existing_host) = parse_auth_host_from_line(trimmed) {
                if existing_host == host {
                    continue;
                }
            }

            lines.push(line.to_string());
        }
    }

    lines.push(format!("//{host}/:_authToken={trimmed_token}"));

    let mut contents = String::new();

    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            contents.push('\n');
        }
        contents.push_str(line);
    }

    fs::write(&rc_path, contents).map_err(|source| SnpmError::WriteFile {
        path: rc_path,
        source,
    })?;

    Ok(())
}

fn default_rc_path() -> PathBuf {
    if let Some(base) = BaseDirs::new() {
        base.home_dir().join(".snpmrc")
    } else {
        PathBuf::from(".snpmrc")
    }
}

fn parse_auth_host_from_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if !trimmed.starts_with("//") {
        return None;
    }

    let eq_idx = trimmed.find('=')?;
    let (key, _) = trimmed.split_at(eq_idx);
    let key = key.trim();
    let rest = key.strip_prefix("//")?;

    let host_and_path = if let Some(prefix) = rest.strip_suffix("/:_authToken") {
        prefix
    } else if let Some(prefix) = rest.strip_suffix(":_authToken") {
        prefix
    } else {
        return None;
    };

    let host = host_and_path
        .split('/')
        .next()
        .unwrap_or("")
        .trim()
        .trim_end_matches('/');

    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

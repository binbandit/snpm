mod auth;
mod entries;

use super::super::types::RegistryConfig;
use super::parse::parse_rc_entry;
use std::fs;
use std::path::Path;

use entries::apply_rc_entry;

pub fn apply_rc_file(path: &Path, config: &mut RegistryConfig) {
    if !path.is_file() {
        return;
    }

    let Ok(data) = fs::read_to_string(path) else {
        return;
    };

    for line in data.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }

        let Some((key, value)) = parse_rc_entry(trimmed) else {
            continue;
        };

        apply_rc_entry(config, key, value);
    }
}

#[cfg(test)]
mod tests;

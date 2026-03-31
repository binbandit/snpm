use crate::SnpmConfig;

use std::env;

pub(super) fn npm_like_registry_for_package(
    config: &SnpmConfig,
    protocol_name: &str,
    name: &str,
) -> String {
    if protocol_name == "npm" {
        return npm_registry_for_package(config, name);
    }

    let key = format!("SNPM_REGISTRY_{}", protocol_name.to_uppercase());
    if let Ok(value) = env::var(&key) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return trimmed.trim_end_matches('/').to_string();
        }
    }

    config.default_registry.clone()
}

fn npm_registry_for_package(config: &SnpmConfig, name: &str) -> String {
    if let Some((scope, _)) = name.split_once('/')
        && scope.starts_with('@')
        && let Some(registry) = config.scoped_registries.get(scope)
    {
        return registry.clone();
    }

    config.default_registry.clone()
}

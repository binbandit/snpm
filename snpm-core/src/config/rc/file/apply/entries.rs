use super::super::super::types::RegistryConfig;
use super::super::super::url::normalize_registry_url;
use super::auth::apply_scoped_auth;
use crate::config::{HoistingMode, parse_package_name_list};

pub(super) fn apply_rc_entry(config: &mut RegistryConfig, key: &str, value: String) {
    if key == "registry" {
        if !value.is_empty() {
            config.default_registry = normalize_registry_url(&value);
        }
        return;
    }

    if matches!(key, "snpm-hoist" | "snpm.hoist" | "snpm_hoist") {
        if let Some(mode) = HoistingMode::parse(&value) {
            config.hoisting = Some(mode);
        }
        return;
    }

    if matches!(
        key,
        "disableGlobalVirtualStoreForPackages"
            | "disable-global-virtual-store-for-packages"
            | "disable_global_virtual_store_for_packages"
    ) {
        config.disable_global_virtual_store_for_packages = Some(parse_package_name_list(&value));
        return;
    }

    if let Some(scope) = key.strip_suffix(":registry") {
        let scope = scope.trim();
        if !scope.is_empty() && !value.is_empty() {
            config
                .scoped
                .insert(scope.to_string(), normalize_registry_url(&value));
        }
        return;
    }

    if let Some(rest) = key.strip_prefix("//") {
        apply_scoped_auth(config, rest, &value);
        return;
    }

    match key {
        "_authToken" if !value.is_empty() => {
            config.default_auth_token = Some(value);
        }
        "_auth" if !value.is_empty() => {
            config.default_auth_token = Some(value);
            config.default_auth_basic = true;
        }
        "always-auth" | "always_auth" | "always.auth" => {
            config.always_auth = is_enabled(&value);
        }
        _ => {}
    }
}

fn is_enabled(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "y" | "on"
    )
}

use snpm_core::{SnpmConfig, console};

use std::collections::BTreeMap;

use super::format::{
    format_auth_status, format_days, format_list, hoisting_label, link_backend_label,
};

pub(super) fn print_paths(config: &SnpmConfig) {
    console::info("paths");
    console::info(&format!("  cache dir: {}", config.cache_dir.display()));
    console::info(&format!("  data dir: {}", config.data_dir.display()));
    console::info(&format!(
        "  packages dir: {}",
        config.packages_dir().display()
    ));
    console::info(&format!(
        "  metadata dir: {}",
        config.metadata_dir().display()
    ));
    println!();
}

pub(super) fn print_registry(config: &SnpmConfig) {
    console::info("registry");
    console::info(&format!("  default: {}", config.default_registry));
    console::info(&format!(
        "  default auth: {}",
        format_auth_status(
            config.default_registry_auth_token.as_deref(),
            config.default_registry_auth_scheme
        )
    ));
    console::info(&format!("  always auth: {}", config.always_auth));
    print_string_map("scoped registries", &config.scoped_registries);
    print_token_map("registry auth", &config.registry_auth);
    println!();
}

pub(super) fn print_install(config: &SnpmConfig) {
    console::info("install");
    console::info(&format!("  hoisting: {}", hoisting_label(config.hoisting)));
    console::info(&format!(
        "  link backend: {}",
        link_backend_label(config.link_backend)
    ));
    console::info(&format!("  strict peers: {}", config.strict_peers));
    console::info(&format!(
        "  frozen lockfile: {}",
        config.frozen_lockfile_default
    ));
    console::info(&format!(
        "  min package age: {}",
        format_days(config.min_package_age_days)
    ));
    console::info(&format!(
        "  min package cache age: {}",
        format_days(config.min_package_cache_age_days)
    ));
    console::info(&format!(
        "  registry concurrency: {}",
        config.registry_concurrency
    ));
    println!();
}

pub(super) fn print_scripts(config: &SnpmConfig) {
    console::info("scripts");
    console::info(&format!(
        "  allow scripts: {}",
        format_list(&config.allow_scripts)
    ));
    println!();
}

pub(super) fn print_logging(config: &SnpmConfig) {
    console::info("logging");
    console::info(&format!("  verbose: {}", config.verbose));
    console::info(&format!(
        "  log file: {}",
        config
            .log_file
            .as_ref()
            .map(|path| path.display().to_string())
            .unwrap_or_else(|| "none".to_string())
    ));
}

fn print_string_map(label: &str, values: &BTreeMap<String, String>) {
    if values.is_empty() {
        console::info(&format!("  {}: none", label));
        return;
    }

    console::info(&format!("  {}:", label));
    for (key, value) in values {
        console::info(&format!("    {}: {}", key, value));
    }
}

fn print_token_map(label: &str, values: &BTreeMap<String, String>) {
    if values.is_empty() {
        console::info(&format!("  {}: none", label));
        return;
    }

    console::info(&format!("  {}:", label));
    for key in values.keys() {
        console::info(&format!("    {}: set", key));
    }
}

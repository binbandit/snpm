use anyhow::Result;
use clap::Args;
use snpm_core::config::AuthScheme;
use snpm_core::{HoistingMode, LinkBackend, SnpmConfig, console};
use std::collections::{BTreeMap, BTreeSet};
use std::env;

#[derive(Args, Debug)]
pub struct ConfigArgs {}

pub async fn run(_args: ConfigArgs, config: &SnpmConfig) -> Result<()> {
    console::header("config", env!("CARGO_PKG_VERSION"));

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

    console::info("scripts");
    console::info(&format!(
        "  allow scripts: {}",
        format_list(&config.allow_scripts)
    ));
    println!();

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

    Ok(())
}

fn format_days(value: Option<u32>) -> String {
    match value {
        Some(days) => format!("{days} days"),
        None => "none".to_string(),
    }
}

fn format_list(values: &BTreeSet<String>) -> String {
    if values.is_empty() {
        return "none".to_string();
    }

    values.iter().cloned().collect::<Vec<_>>().join(", ")
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
    for (key, _value) in values {
        console::info(&format!("    {}: set", key));
    }
}

fn format_auth_status(token: Option<&str>, scheme: AuthScheme) -> String {
    if token.is_none() {
        return "none".to_string();
    }

    format!("set ({})", auth_scheme_label(scheme))
}

fn auth_scheme_label(scheme: AuthScheme) -> &'static str {
    match scheme {
        AuthScheme::Bearer => "bearer",
        AuthScheme::Basic => "basic",
    }
}

fn hoisting_label(value: HoistingMode) -> &'static str {
    match value {
        HoistingMode::None => "none",
        HoistingMode::SingleVersion => "single-version",
        HoistingMode::All => "all",
    }
}

fn link_backend_label(value: LinkBackend) -> &'static str {
    match value {
        LinkBackend::Auto => "auto",
        LinkBackend::Hardlink => "hardlink",
        LinkBackend::Symlink => "symlink",
        LinkBackend::Copy => "copy",
    }
}

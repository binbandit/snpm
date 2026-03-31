use snpm_core::config::AuthScheme;
use snpm_core::{HoistingMode, LinkBackend};

use std::collections::BTreeSet;

pub(super) fn format_days(value: Option<u32>) -> String {
    match value {
        Some(days) => format!("{days} days"),
        None => "none".to_string(),
    }
}

pub(super) fn format_list(values: &BTreeSet<String>) -> String {
    if values.is_empty() {
        return "none".to_string();
    }

    values.iter().cloned().collect::<Vec<_>>().join(", ")
}

pub(super) fn format_auth_status(token: Option<&str>, scheme: AuthScheme) -> String {
    if token.is_none() {
        return "none".to_string();
    }

    format!("set ({})", auth_scheme_label(scheme))
}

pub(super) fn hoisting_label(value: HoistingMode) -> &'static str {
    match value {
        HoistingMode::None => "none",
        HoistingMode::SingleVersion => "single-version",
        HoistingMode::All => "all",
    }
}

pub(super) fn link_backend_label(value: LinkBackend) -> &'static str {
    match value {
        LinkBackend::Auto => "auto",
        LinkBackend::Reflink => "reflink",
        LinkBackend::Hardlink => "hardlink",
        LinkBackend::Symlink => "symlink",
        LinkBackend::Copy => "copy",
    }
}

fn auth_scheme_label(scheme: AuthScheme) -> &'static str {
    match scheme {
        AuthScheme::Bearer => "bearer",
        AuthScheme::Basic => "basic",
    }
}

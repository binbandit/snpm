use super::types::{AuditAdvisory, AuditOptions, Severity, VulnerabilityCounts};
use std::collections::HashMap;

pub(super) fn filter_advisories(
    advisories: &HashMap<String, AuditAdvisory>,
    options: &AuditOptions,
) -> Vec<AuditAdvisory> {
    let mut filtered: Vec<_> = advisories
        .values()
        .filter(|advisory| matches_options(advisory, options))
        .cloned()
        .collect();

    filtered.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then_with(|| left.module_name.cmp(&right.module_name))
    });

    filtered
}

fn matches_options(advisory: &AuditAdvisory, options: &AuditOptions) -> bool {
    if let Some(threshold) = options.audit_level
        && advisory.severity < threshold
    {
        return false;
    }

    if advisory
        .cves
        .iter()
        .any(|cve| options.ignore_cves.contains(cve))
    {
        return false;
    }

    if let Some(ghsa) = &advisory.github_advisory_id
        && options
            .ignore_ghsas
            .iter()
            .any(|ignored| ignored.eq_ignore_ascii_case(ghsa))
    {
        return false;
    }

    if options.ignore_unfixable && is_unfixable(&advisory.patched_versions) {
        return false;
    }

    if !options.packages.is_empty() && !options.packages.contains(&advisory.module_name) {
        return false;
    }

    true
}

pub(super) fn calculate_counts(advisories: &[AuditAdvisory]) -> VulnerabilityCounts {
    let mut counts = VulnerabilityCounts::default();

    for advisory in advisories {
        match advisory.severity {
            Severity::Info => counts.info += 1,
            Severity::Low => counts.low += 1,
            Severity::Moderate => counts.moderate += 1,
            Severity::High => counts.high += 1,
            Severity::Critical => counts.critical += 1,
        }
    }

    counts
}

pub(super) fn is_unfixable(patched_versions: &str) -> bool {
    patched_versions.is_empty()
        || patched_versions == "<0.0.0"
        || patched_versions == "No fix available"
}

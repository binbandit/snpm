use super::types::{AuditAdvisory, AuditOptions, Severity, VulnerabilityCounts};
use crate::lockfile::Lockfile;
use std::collections::{BTreeMap, HashMap, HashSet};

pub(super) fn collect_dev_only_names(
    lockfile: &Lockfile,
    dev_dependencies: &BTreeMap<String, String>,
) -> HashSet<String> {
    let mut prod_reachable = HashSet::new();
    let mut dev_reachable = HashSet::new();

    for name in lockfile.root.dependencies.keys() {
        let reachable = if dev_dependencies.contains_key(name) {
            &mut dev_reachable
        } else {
            &mut prod_reachable
        };
        walk_deps(lockfile, name, reachable);
    }

    dev_reachable.difference(&prod_reachable).cloned().collect()
}

fn walk_deps(lockfile: &Lockfile, name: &str, visited: &mut HashSet<String>) {
    if !visited.insert(name.to_string()) {
        return;
    }

    for package in lockfile.packages.values() {
        if package.name == name {
            for dependency in package.dependencies.keys() {
                walk_deps(lockfile, dependency, visited);
            }
        }
    }
}

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
        && options.ignore_ghsas.contains(ghsa)
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

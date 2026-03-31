use std::collections::HashSet;

use super::counts::VulnerabilityCounts;
use super::severity::Severity;
use super::wire::AuditAdvisory;

#[derive(Debug, Clone, Default)]
pub struct AuditOptions {
    pub audit_level: Option<Severity>,
    pub production_only: bool,
    pub dev_only: bool,
    pub packages: Vec<String>,
    pub ignore_cves: HashSet<String>,
    pub ignore_ghsas: HashSet<String>,
    pub ignore_unfixable: bool,
}

#[derive(Debug, Clone)]
pub struct AuditResult {
    pub advisories: Vec<AuditAdvisory>,
    pub counts: VulnerabilityCounts,
    pub total_packages: usize,
    pub project_name: String,
    pub workspace_member: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FixResult {
    pub fixed: Vec<FixedVulnerability>,
    pub unfixable: Vec<UnfixableVulnerability>,
}

#[derive(Debug, Clone)]
pub struct FixedVulnerability {
    pub package: String,
    pub from_version: String,
    pub to_version: String,
    pub advisory_id: u64,
    pub severity: Severity,
}

#[derive(Debug, Clone)]
pub struct UnfixableVulnerability {
    pub package: String,
    pub version: String,
    pub advisory_id: u64,
    pub severity: Severity,
    pub reason: String,
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::counts::VulnerabilityCounts;
use super::severity::Severity;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResponse {
    #[serde(default)]
    pub actions: Vec<AuditAction>,
    #[serde(default)]
    pub advisories: HashMap<String, AuditAdvisory>,
    #[serde(default)]
    pub metadata: AuditMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditAction {
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub module: String,
    #[serde(default)]
    pub resolves: Vec<AuditResolve>,
    #[serde(default)]
    pub target: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditResolve {
    pub id: u64,
    pub path: String,
    #[serde(default)]
    pub dev: bool,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub bundled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditAdvisory {
    pub id: u64,
    #[serde(default)]
    pub created: Option<String>,
    #[serde(default)]
    pub updated: Option<String>,
    #[serde(default)]
    pub title: String,
    pub module_name: String,
    #[serde(default)]
    pub cves: Vec<String>,
    #[serde(default)]
    pub vulnerable_versions: String,
    #[serde(default)]
    pub patched_versions: String,
    #[serde(default)]
    pub overview: String,
    #[serde(default)]
    pub recommendation: String,
    #[serde(default)]
    pub severity: Severity,
    #[serde(default)]
    pub cwe: Option<String>,
    #[serde(default)]
    pub github_advisory_id: Option<String>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub findings: Vec<AuditFinding>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditFinding {
    pub version: String,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub dev: bool,
    #[serde(default)]
    pub optional: bool,
    #[serde(default)]
    pub bundled: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuditMetadata {
    #[serde(default)]
    pub vulnerabilities: VulnerabilityCounts,
    #[serde(default)]
    pub dependencies: u64,
    #[serde(default, rename = "devDependencies")]
    pub dev_dependencies: u64,
    #[serde(default, rename = "optionalDependencies")]
    pub optional_dependencies: u64,
    #[serde(default, rename = "totalDependencies")]
    pub total_dependencies: u64,
}

use crate::config::AuthScheme;
use crate::lockfile::{self, Lockfile};
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};

// ============================================================================
// Request types (sent to the npm audit API)
// ============================================================================

#[derive(Debug, Serialize)]
struct AuditNode {
    version: Option<String>,
    integrity: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    requires: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dependencies: Option<BTreeMap<String, AuditNode>>,
    dev: bool,
}

#[derive(Debug, Serialize)]
struct AuditRequest {
    name: String,
    version: String,
    requires: BTreeMap<String, String>,
    dependencies: BTreeMap<String, AuditNode>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    install: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    remove: Vec<String>,
    metadata: AuditMetadataRequest,
}

#[derive(Debug, Serialize)]
struct AuditMetadataRequest {
    node_version: String,
    npm_version: String,
    platform: String,
}

// ============================================================================
// Response types (received from the npm audit API)
// ============================================================================

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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VulnerabilityCounts {
    #[serde(default)]
    pub info: u64,
    #[serde(default)]
    pub low: u64,
    #[serde(default)]
    pub moderate: u64,
    #[serde(default)]
    pub high: u64,
    #[serde(default)]
    pub critical: u64,
}

impl VulnerabilityCounts {
    pub fn total(&self) -> u64 {
        self.info + self.low + self.moderate + self.high + self.critical
    }

    pub fn above_threshold(&self, threshold: Severity) -> u64 {
        match threshold {
            Severity::Info => self.total(),
            Severity::Low => self.low + self.moderate + self.high + self.critical,
            Severity::Moderate => self.moderate + self.high + self.critical,
            Severity::High => self.high + self.critical,
            Severity::Critical => self.critical,
        }
    }

    pub fn merge(&mut self, other: &VulnerabilityCounts) {
        self.info += other.info;
        self.low += other.low;
        self.moderate += other.moderate;
        self.high += other.high;
        self.critical += other.critical;
    }
}

// ============================================================================
// Severity
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Severity {
    #[default]
    Info,
    Low,
    Moderate,
    High,
    Critical,
}

impl Severity {
    pub fn as_str(&self) -> &'static str {
        match self {
            Severity::Info => "info",
            Severity::Low => "low",
            Severity::Moderate => "moderate",
            Severity::High => "high",
            Severity::Critical => "critical",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl Serialize for Severity {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Severity {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(match s.to_lowercase().as_str() {
            "info" => Severity::Info,
            "low" => Severity::Low,
            "moderate" => Severity::Moderate,
            "high" => Severity::High,
            "critical" => Severity::Critical,
            _ => Severity::Info,
        })
    }
}

impl std::str::FromStr for Severity {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "info" => Ok(Severity::Info),
            "low" => Ok(Severity::Low),
            "moderate" => Ok(Severity::Moderate),
            "high" => Ok(Severity::High),
            "critical" => Ok(Severity::Critical),
            _ => Err(format!(
                "invalid severity '{}' (expected: info, low, moderate, high, critical)",
                s
            )),
        }
    }
}

// ============================================================================
// SARIF output (GitHub/GitLab security tab integration)
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct SarifReport {
    #[serde(rename = "$schema")]
    pub schema: String,
    pub version: String,
    pub runs: Vec<SarifRun>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SarifRun {
    pub tool: SarifTool,
    pub results: Vec<SarifResult>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SarifTool {
    pub driver: SarifDriver,
}

#[derive(Debug, Clone, Serialize)]
pub struct SarifDriver {
    pub name: String,
    pub version: String,
    #[serde(rename = "informationUri")]
    pub information_uri: String,
    pub rules: Vec<SarifRule>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SarifRule {
    pub id: String,
    pub name: String,
    #[serde(rename = "shortDescription")]
    pub short_description: SarifMessage,
    #[serde(rename = "fullDescription")]
    pub full_description: SarifMessage,
    #[serde(rename = "helpUri")]
    pub help_uri: Option<String>,
    #[serde(rename = "defaultConfiguration")]
    pub default_configuration: SarifRuleConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct SarifRuleConfig {
    pub level: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SarifMessage {
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SarifResult {
    #[serde(rename = "ruleId")]
    pub rule_id: String,
    pub level: String,
    pub message: SarifMessage,
    pub locations: Vec<SarifLocation>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SarifLocation {
    #[serde(rename = "physicalLocation")]
    pub physical_location: SarifPhysicalLocation,
}

#[derive(Debug, Clone, Serialize)]
pub struct SarifPhysicalLocation {
    #[serde(rename = "artifactLocation")]
    pub artifact_location: SarifArtifactLocation,
}

#[derive(Debug, Clone, Serialize)]
pub struct SarifArtifactLocation {
    pub uri: String,
}

const SARIF_SCHEMA: &str = "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json";

// ============================================================================
// Options and result
// ============================================================================

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
    /// Filtered advisories (respects options)
    pub advisories: Vec<AuditAdvisory>,
    /// Counts after filtering
    pub counts: VulnerabilityCounts,
    /// Number of packages that were scanned
    pub total_packages: usize,
    /// Project name from package.json
    pub project_name: String,
    /// Relative path within workspace (None for standalone projects)
    pub workspace_member: Option<String>,
}

impl AuditResult {
    pub fn to_sarif(&self) -> SarifReport {
        let mut rules = Vec::new();
        let mut results = Vec::new();

        for advisory in &self.advisories {
            let rule_id = format!("SNPM-{}", advisory.id);
            let level = match advisory.severity {
                Severity::Critical | Severity::High => "error",
                Severity::Moderate => "warning",
                _ => "note",
            };

            rules.push(SarifRule {
                id: rule_id.clone(),
                name: advisory.title.clone(),
                short_description: SarifMessage {
                    text: format!(
                        "{} in {} ({})",
                        advisory.title, advisory.module_name, advisory.severity
                    ),
                },
                full_description: SarifMessage {
                    text: advisory.overview.clone(),
                },
                help_uri: advisory.url.clone(),
                default_configuration: SarifRuleConfig {
                    level: level.to_string(),
                },
            });

            for finding in &advisory.findings {
                for path in &finding.paths {
                    let fix_text = if is_unfixable(&advisory.patched_versions) {
                        "no fix available".to_string()
                    } else {
                        advisory.patched_versions.clone()
                    };

                    results.push(SarifResult {
                        rule_id: rule_id.clone(),
                        level: level.to_string(),
                        message: SarifMessage {
                            text: format!(
                                "{} {} has {} vulnerability: {}. Fix: upgrade to {}",
                                advisory.module_name,
                                finding.version,
                                advisory.severity,
                                advisory.title,
                                fix_text,
                            ),
                        },
                        locations: vec![SarifLocation {
                            physical_location: SarifPhysicalLocation {
                                artifact_location: SarifArtifactLocation {
                                    uri: format!("package.json#{}", path.replace('>', "/")),
                                },
                            },
                        }],
                    });
                }
            }
        }

        SarifReport {
            schema: SARIF_SCHEMA.to_string(),
            version: "2.1.0".to_string(),
            runs: vec![SarifRun {
                tool: SarifTool {
                    driver: SarifDriver {
                        name: "snpm-audit".to_string(),
                        version: env!("CARGO_PKG_VERSION").to_string(),
                        information_uri: "https://github.com/nicolo-ribaudo/snpm".to_string(),
                        rules,
                    },
                },
                results,
            }],
        }
    }

    /// Serialize the filtered audit result as JSON (not the raw response)
    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::json!({
            "vulnerabilities": self.counts,
            "totalPackages": self.total_packages,
            "advisories": self.advisories,
        })
    }
}

// ============================================================================
// Public API
// ============================================================================

/// Audit a single project for security vulnerabilities.
pub async fn audit(
    config: &SnpmConfig,
    project: &Project,
    options: &AuditOptions,
) -> Result<AuditResult> {
    let lockfile_path = project.root.join("snpm-lock.yaml");
    if !lockfile_path.exists() {
        return Err(SnpmError::AuditLockfileRequired);
    }

    let lockfile = lockfile::read(&lockfile_path)?;
    let project_name = project
        .manifest
        .name
        .clone()
        .unwrap_or_else(|| "unknown".to_string());

    let dev_names = collect_dev_only_names(&lockfile, &project.manifest.dev_dependencies);
    audit_lockfile(config, &lockfile, &project_name, None, &dev_names, options).await
}

/// Audit every project in a workspace. Uses the root lockfile and makes a
/// single API call (the npm audit API accepts the full dependency tree).
pub async fn audit_workspace(
    config: &SnpmConfig,
    workspace: &Workspace,
    options: &AuditOptions,
) -> Result<Vec<AuditResult>> {
    // Workspace has a single root lockfile
    let lockfile_path = workspace.root.join("snpm-lock.yaml");
    if !lockfile_path.exists() {
        return Err(SnpmError::AuditLockfileRequired);
    }

    let lockfile = lockfile::read(&lockfile_path)?;

    // Collect dev dependency names from all workspace members
    let mut all_dev_names = HashSet::new();
    for project in &workspace.projects {
        for name in project.manifest.dev_dependencies.keys() {
            all_dev_names.insert(name.clone());
        }
    }

    let dev_names = collect_dev_only_names(
        &lockfile,
        &all_dev_names
            .into_iter()
            .map(|n| (n, String::new()))
            .collect(),
    );

    let workspace_name = workspace
        .projects
        .first()
        .and_then(|p| p.manifest.name.clone())
        .unwrap_or_else(|| "workspace".to_string());

    let result = audit_lockfile(
        config,
        &lockfile,
        &workspace_name,
        Some(".".to_string()),
        &dev_names,
        options,
    )
    .await?;

    Ok(vec![result])
}

// ============================================================================
// Internal implementation
// ============================================================================

/// Walk the dependency graph from dev-only roots to find all transitively
/// dev-only packages. A package is dev-only if it is *only* reachable through
/// devDependencies roots.
fn collect_dev_only_names(
    lockfile: &Lockfile,
    dev_dependencies: &BTreeMap<String, String>,
) -> HashSet<String> {
    // Packages reachable from production roots
    let mut prod_reachable: HashSet<String> = HashSet::new();
    // Packages reachable from dev roots
    let mut dev_reachable: HashSet<String> = HashSet::new();

    // Walk from each root dependency
    for name in lockfile.root.dependencies.keys() {
        let is_dev_root = dev_dependencies.contains_key(name);
        let target = if is_dev_root {
            &mut dev_reachable
        } else {
            &mut prod_reachable
        };
        walk_deps(lockfile, name, target);
    }

    // Dev-only = reachable from dev roots but NOT reachable from prod roots
    dev_reachable.difference(&prod_reachable).cloned().collect()
}

fn walk_deps(lockfile: &Lockfile, name: &str, visited: &mut HashSet<String>) {
    if !visited.insert(name.to_string()) {
        return;
    }

    // Find all versions of this package in the lockfile and walk their deps
    for pkg in lockfile.packages.values() {
        if pkg.name == name {
            for dep_name in pkg.dependencies.keys() {
                walk_deps(lockfile, dep_name, visited);
            }
        }
    }
}

async fn audit_lockfile(
    config: &SnpmConfig,
    lockfile: &Lockfile,
    project_name: &str,
    workspace_member: Option<String>,
    dev_names: &HashSet<String>,
    options: &AuditOptions,
) -> Result<AuditResult> {
    let client = Client::builder()
        .build()
        .map_err(|source| SnpmError::HttpClient { source })?;

    let request = build_audit_request(lockfile, project_name, dev_names, options);
    let total_packages = lockfile.packages.len();

    let registry = config.default_registry.trim_end_matches('/');
    let audit_url = format!("{}/-/npm/v1/security/audits", registry);

    let mut req = client
        .post(&audit_url)
        .header("Content-Type", "application/json")
        .json(&request);

    // Add auth with correct scheme (Bearer vs Basic)
    if let Some(token) = config.auth_token_for_url(&audit_url) {
        let scheme = config.auth_scheme_for_url(&audit_url);
        let header_value = match scheme {
            AuthScheme::Basic => format!("Basic {}", token),
            AuthScheme::Bearer => format!("Bearer {}", token),
        };
        req = req.header("Authorization", header_value);
    }

    let response = req.send().await.map_err(|source| SnpmError::Http {
        url: audit_url.clone(),
        source,
    })?;

    if response.status() == 404 {
        return Err(SnpmError::AuditEndpointNotAvailable {
            registry: registry.to_string(),
        });
    }

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(SnpmError::AuditFailed {
            reason: format!("Registry returned {}: {}", status, body),
        });
    }

    let audit_response: AuditResponse =
        response.json().await.map_err(|source| SnpmError::Http {
            url: audit_url,
            source,
        })?;

    let filtered = filter_advisories(&audit_response.advisories, options);
    let counts = calculate_counts(&filtered);

    Ok(AuditResult {
        advisories: filtered,
        counts,
        total_packages,
        project_name: project_name.to_string(),
        workspace_member,
    })
}

fn build_audit_request(
    lockfile: &Lockfile,
    project_name: &str,
    dev_names: &HashSet<String>,
    options: &AuditOptions,
) -> AuditRequest {
    let mut requires = BTreeMap::new();
    let mut dependencies = BTreeMap::new();

    for (name, root_dep) in &lockfile.root.dependencies {
        requires.insert(name.clone(), root_dep.requested.clone());
    }

    for pkg in lockfile.packages.values() {
        if !options.packages.is_empty() && !options.packages.contains(&pkg.name) {
            continue;
        }

        let is_dev = dev_names.contains(&pkg.name);

        if options.production_only && is_dev {
            continue;
        }
        if options.dev_only && !is_dev {
            continue;
        }

        let mut pkg_requires = BTreeMap::new();
        for (dep_name, dep_key) in &pkg.dependencies {
            if let Some((_, version)) = split_dep_key(dep_key) {
                pkg_requires.insert(dep_name.clone(), version);
            }
        }

        let node = AuditNode {
            version: Some(pkg.version.clone()),
            integrity: pkg.integrity.clone(),
            requires: if pkg_requires.is_empty() {
                None
            } else {
                Some(pkg_requires)
            },
            dependencies: None,
            dev: is_dev,
        };

        dependencies.insert(pkg.name.clone(), node);
    }

    AuditRequest {
        name: project_name.to_string(),
        version: "0.0.0".to_string(),
        requires,
        dependencies,
        install: vec![],
        remove: vec![],
        metadata: AuditMetadataRequest {
            node_version: "20.0.0".to_string(),
            npm_version: "10.0.0".to_string(),
            platform: std::env::consts::OS.to_string(),
        },
    }
}

fn filter_advisories(
    advisories: &HashMap<String, AuditAdvisory>,
    options: &AuditOptions,
) -> Vec<AuditAdvisory> {
    let mut filtered: Vec<AuditAdvisory> = advisories
        .values()
        .filter(|advisory| {
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
        })
        .cloned()
        .collect();

    // Critical first, then alphabetical within each severity
    filtered.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then_with(|| a.module_name.cmp(&b.module_name))
    });

    filtered
}

fn calculate_counts(advisories: &[AuditAdvisory]) -> VulnerabilityCounts {
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

fn is_unfixable(patched_versions: &str) -> bool {
    patched_versions.is_empty()
        || patched_versions == "<0.0.0"
        || patched_versions == "No fix available"
}

fn split_dep_key(key: &str) -> Option<(String, String)> {
    let idx = key.rfind('@')?;
    let (name, version_part) = key.split_at(idx);
    let version = version_part.trim_start_matches('@').to_string();
    Some((name.to_string(), version))
}

// ============================================================================
// Fix
// ============================================================================

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

/// Identify fixable and unfixable vulnerabilities. Records which packages
/// would need to be re-resolved to patched versions.
pub async fn fix(
    config: &SnpmConfig,
    project: &Project,
    options: &AuditOptions,
) -> Result<FixResult> {
    let audit_result = audit(config, project, options).await?;

    let mut fixed = Vec::new();
    let mut unfixable = Vec::new();

    for advisory in &audit_result.advisories {
        if is_unfixable(&advisory.patched_versions) {
            for finding in &advisory.findings {
                unfixable.push(UnfixableVulnerability {
                    package: advisory.module_name.clone(),
                    version: finding.version.clone(),
                    advisory_id: advisory.id,
                    severity: advisory.severity,
                    reason: "No patched version available".to_string(),
                });
            }
        } else {
            for finding in &advisory.findings {
                fixed.push(FixedVulnerability {
                    package: advisory.module_name.clone(),
                    from_version: finding.version.clone(),
                    to_version: advisory.patched_versions.clone(),
                    advisory_id: advisory.id,
                    severity: advisory.severity,
                });
            }
        }
    }

    Ok(FixResult { fixed, unfixable })
}

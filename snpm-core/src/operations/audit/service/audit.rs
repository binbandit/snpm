use super::super::filter::{calculate_counts, filter_advisories};
use super::super::request::{AuditPathIndex, build_audit_path_index, build_audit_request};
use super::super::types::{AuditAdvisory, AuditFinding, AuditOptions, AuditResult, Severity};
use crate::lockfile::{self, Lockfile};
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace, http};
use serde::Deserialize;
use snpm_semver::{RangeSet, parse_version};

use std::collections::{BTreeMap, HashMap, HashSet};

pub async fn audit(
    config: &SnpmConfig,
    project: &Project,
    options: &AuditOptions,
) -> Result<AuditResult> {
    let lockfile = read_audit_lockfile(&project.root.join("snpm-lock.yaml"))?;
    let project_name = project
        .manifest
        .name
        .clone()
        .unwrap_or_else(|| "unknown".to_string());
    let dev_root_names = project.manifest.dev_dependencies.keys().cloned().collect();
    audit_lockfile(
        config,
        &lockfile,
        &project_name,
        None,
        &dev_root_names,
        options,
    )
    .await
}

pub async fn audit_workspace(
    config: &SnpmConfig,
    workspace: &Workspace,
    options: &AuditOptions,
) -> Result<Vec<AuditResult>> {
    let lockfile = read_audit_lockfile(&workspace.root.join("snpm-lock.yaml"))?;
    let workspace_name = workspace
        .projects
        .first()
        .and_then(|project| project.manifest.name.clone())
        .unwrap_or_else(|| "workspace".to_string());
    let dev_root_names = workspace_dev_root_names(workspace);

    let result = audit_lockfile(
        config,
        &lockfile,
        &workspace_name,
        Some(".".to_string()),
        &dev_root_names,
        options,
    )
    .await?;

    Ok(vec![result])
}

pub(super) async fn audit_lockfile(
    config: &SnpmConfig,
    lockfile: &Lockfile,
    project_name: &str,
    workspace_member: Option<String>,
    dev_root_names: &HashSet<String>,
    options: &AuditOptions,
) -> Result<AuditResult> {
    let client = http::create_client()?;
    let request = build_audit_request(lockfile, dev_root_names, options);
    let audit_url = audit_url(config);

    let mut request_builder = client
        .post(&audit_url)
        .header("Content-Type", "application/json")
        .json(&request.request);

    if let Some(header_value) = config.authorization_header_for_url(&audit_url) {
        request_builder = request_builder.header("Authorization", header_value);
    }

    let response = request_builder
        .send()
        .await
        .map_err(|source| SnpmError::Http {
            url: audit_url.clone(),
            source,
        })?;

    if response.status() == 404 {
        return Err(SnpmError::AuditEndpointNotAvailable {
            registry: config.default_registry.trim_end_matches('/').to_string(),
        });
    }

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(SnpmError::AuditFailed {
            reason: format!("Registry returned {}: {}", status, body),
        });
    }

    let body = response.text().await.unwrap_or_default();
    let bulk_response: BulkAuditResponse =
        serde_json::from_str(&body).map_err(|error| SnpmError::AuditFailed {
            reason: format!("Registry returned invalid bulk audit response: {error}"),
        })?;
    let vulnerable_names = bulk_response.keys().cloned().collect::<HashSet<_>>();
    let path_index = build_audit_path_index(lockfile, dev_root_names, options, &vulnerable_names);
    let advisories = filter_advisories(
        &build_audit_advisories(&bulk_response, &path_index),
        options,
    );

    Ok(AuditResult {
        counts: calculate_counts(&advisories),
        advisories,
        total_packages: request.total_packages,
        project_name: project_name.to_string(),
        workspace_member,
    })
}

fn read_audit_lockfile(path: &std::path::Path) -> Result<Lockfile> {
    if !path.exists() {
        return Err(SnpmError::AuditLockfileRequired);
    }

    lockfile::read(path)
}

fn workspace_dev_root_names(workspace: &Workspace) -> HashSet<String> {
    workspace
        .projects
        .iter()
        .flat_map(|project| project.manifest.dev_dependencies.keys().cloned())
        .collect()
}

fn audit_url(config: &SnpmConfig) -> String {
    let registry = config.default_registry.trim_end_matches('/');
    format!("{registry}/-/npm/v1/security/advisories/bulk")
}

type BulkAuditResponse = HashMap<String, Vec<BulkAdvisory>>;

#[derive(Debug, Clone, Deserialize)]
struct BulkAdvisory {
    id: u64,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    title: String,
    severity: String,
    vulnerable_versions: String,
    #[serde(default)]
    cwe: Option<BulkCwe>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum BulkCwe {
    One(String),
    Many(Vec<String>),
}

fn build_audit_advisories(
    bulk_response: &BulkAuditResponse,
    path_index: &AuditPathIndex,
) -> HashMap<String, AuditAdvisory> {
    let mut advisories = HashMap::new();

    for (module_name, entries) in bulk_response {
        let Some(by_version) = path_index.get(module_name) else {
            continue;
        };

        for advisory in entries {
            let Ok(severity) = advisory.severity.parse::<Severity>() else {
                continue;
            };
            let findings = build_findings(by_version, &advisory.vulnerable_versions);
            if findings.is_empty() {
                continue;
            }

            advisories.insert(
                advisory.id.to_string(),
                AuditAdvisory {
                    id: advisory.id,
                    created: None,
                    updated: None,
                    title: advisory.title.clone(),
                    module_name: module_name.clone(),
                    cves: Vec::new(),
                    vulnerable_versions: advisory.vulnerable_versions.clone(),
                    patched_versions: infer_patched_versions(&advisory.vulnerable_versions)
                        .unwrap_or_default(),
                    overview: String::new(),
                    recommendation: String::new(),
                    severity,
                    cwe: advisory.cwe.as_ref().and_then(BulkCwe::as_string),
                    github_advisory_id: advisory.url.as_deref().and_then(derive_github_advisory_id),
                    url: advisory.url.clone(),
                    findings,
                },
            );
        }
    }

    advisories
}

fn build_findings(
    by_version: &BTreeMap<String, super::super::request::PathInfo>,
    vulnerable_versions: &str,
) -> Vec<AuditFinding> {
    let Ok(range) = RangeSet::parse(vulnerable_versions) else {
        return Vec::new();
    };
    let mut findings = Vec::new();

    for (version, info) in by_version {
        let Ok(parsed_version) = parse_version(version) else {
            continue;
        };
        if !range.matches(&parsed_version) {
            continue;
        }

        findings.push(AuditFinding {
            version: version.clone(),
            paths: info.paths.clone(),
            dev: info.dev,
            optional: info.optional,
            bundled: false,
        });
    }

    findings
}

fn infer_patched_versions(vulnerable_versions: &str) -> Option<String> {
    let comparator = vulnerable_versions
        .trim()
        .rsplit_once('<')
        .map(|(_, tail)| tail.trim())?;

    if let Some(version) = comparator.strip_prefix('=') {
        let version = version.trim();
        return Some(
            next_patch_version(version)
                .map(|next| format!(">={next}"))
                .unwrap_or_else(|| format!(">{version}")),
        );
    }

    if comparator.is_empty() {
        return None;
    }

    Some(format!(">={comparator}"))
}

fn next_patch_version(version: &str) -> Option<String> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    let patch = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let patch = patch.parse::<u64>().ok()?;

    Some(format!("{major}.{minor}.{}", patch + 1))
}

fn derive_github_advisory_id(url: &str) -> Option<String> {
    let segment = url
        .split('/')
        .find(|segment| segment.to_ascii_lowercase().starts_with("ghsa-"))?;
    if segment.len() <= 5 {
        return None;
    }
    let suffix = segment[5..].to_ascii_lowercase();
    Some(format!("GHSA-{suffix}"))
}

impl BulkCwe {
    fn as_string(&self) -> Option<String> {
        match self {
            BulkCwe::One(value) if !value.is_empty() => Some(value.clone()),
            BulkCwe::Many(values) if !values.is_empty() => Some(values.join(", ")),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lockfile::{LockPackage, LockRoot, LockRootDependency, Lockfile};
    use std::collections::BTreeMap;

    #[test]
    fn infer_patched_versions_handles_lt_and_lte_ranges() {
        assert_eq!(
            infer_patched_versions(">=1.0.0 <2.0.0"),
            Some(">=2.0.0".to_string())
        );
        assert_eq!(
            infer_patched_versions("<=1.2.3"),
            Some(">=1.2.4".to_string())
        );
    }

    #[test]
    fn build_audit_advisories_skips_non_matching_versions() {
        let bulk = HashMap::from([(
            "shared".to_string(),
            vec![BulkAdvisory {
                id: 42,
                url: Some("https://github.com/advisories/GHSA-abcd-1234-efgh".to_string()),
                title: "Test advisory".to_string(),
                severity: "high".to_string(),
                vulnerable_versions: "<1.5.0".to_string(),
                cwe: Some(BulkCwe::Many(vec![
                    "CWE-79".to_string(),
                    "CWE-89".to_string(),
                ])),
            }],
        )]);
        let path_index = BTreeMap::from([(
            "shared".to_string(),
            BTreeMap::from([
                (
                    "1.0.0".to_string(),
                    crate::operations::audit::request::PathInfo {
                        paths: vec!["prod-root>shared".to_string()],
                        dev: false,
                        optional: false,
                    },
                ),
                (
                    "2.0.0".to_string(),
                    crate::operations::audit::request::PathInfo {
                        paths: vec!["prod-root>shared".to_string()],
                        dev: false,
                        optional: false,
                    },
                ),
            ]),
        )]);

        let advisories = build_audit_advisories(&bulk, &path_index);
        let advisory = &advisories["42"];

        assert_eq!(advisory.findings.len(), 1);
        assert_eq!(advisory.findings[0].version, "1.0.0");
        assert_eq!(
            advisory.github_advisory_id.as_deref(),
            Some("GHSA-abcd-1234-efgh")
        );
        assert_eq!(advisory.cwe.as_deref(), Some("CWE-79, CWE-89"));
    }

    #[test]
    fn audit_request_total_packages_matches_unique_versions() {
        let lockfile = Lockfile {
            version: 1,
            root: LockRoot {
                dependencies: BTreeMap::from([(
                    "shared".to_string(),
                    LockRootDependency {
                        requested: "^1.0.0".to_string(),
                        package: None,
                        version: Some("1.0.0".to_string()),
                        optional: false,
                    },
                )]),
            },
            packages: BTreeMap::from([(
                "shared@1.0.0".to_string(),
                LockPackage {
                    name: "shared".to_string(),
                    version: "1.0.0".to_string(),
                    tarball: String::new(),
                    integrity: None,
                    dependencies: BTreeMap::new(),
                    bundled_dependencies: None,
                    has_bin: false,
                    bin: None,
                },
            )]),
        };

        let request = build_audit_request(&lockfile, &HashSet::new(), &AuditOptions::default());
        assert_eq!(request.total_packages, 1);
    }
}

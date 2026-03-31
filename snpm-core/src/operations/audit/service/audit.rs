use super::super::filter::{calculate_counts, collect_dev_only_names, filter_advisories};
use super::super::request::build_audit_request;
use super::super::types::{AuditOptions, AuditResponse, AuditResult};
use crate::lockfile::{self, Lockfile};
use crate::{Project, Result, SnpmConfig, SnpmError, Workspace, http};

use std::collections::{BTreeMap, HashSet};

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

    let dev_names = collect_dev_only_names(&lockfile, &project.manifest.dev_dependencies);
    audit_lockfile(config, &lockfile, &project_name, None, &dev_names, options).await
}

pub async fn audit_workspace(
    config: &SnpmConfig,
    workspace: &Workspace,
    options: &AuditOptions,
) -> Result<Vec<AuditResult>> {
    let lockfile = read_audit_lockfile(&workspace.root.join("snpm-lock.yaml"))?;
    let dev_names = workspace_dev_names(workspace, &lockfile);
    let workspace_name = workspace
        .projects
        .first()
        .and_then(|project| project.manifest.name.clone())
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

pub(super) async fn audit_lockfile(
    config: &SnpmConfig,
    lockfile: &Lockfile,
    project_name: &str,
    workspace_member: Option<String>,
    dev_names: &HashSet<String>,
    options: &AuditOptions,
) -> Result<AuditResult> {
    let client = http::create_client()?;
    let total_packages = lockfile.packages.len();
    let request = build_audit_request(lockfile, project_name, dev_names, options);
    let audit_url = audit_url(config);

    let mut request_builder = client
        .post(&audit_url)
        .header("Content-Type", "application/json")
        .json(&request);

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

    let audit_response: AuditResponse =
        response.json().await.map_err(|source| SnpmError::Http {
            url: audit_url,
            source,
        })?;

    let advisories = filter_advisories(&audit_response.advisories, options);

    Ok(AuditResult {
        counts: calculate_counts(&advisories),
        advisories,
        total_packages,
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

fn workspace_dev_names(workspace: &Workspace, lockfile: &Lockfile) -> HashSet<String> {
    let mut all_dev_names = HashSet::new();

    for project in &workspace.projects {
        for name in project.manifest.dev_dependencies.keys() {
            all_dev_names.insert(name.clone());
        }
    }

    collect_dev_only_names(
        lockfile,
        &all_dev_names
            .into_iter()
            .map(|name| (name, String::new()))
            .collect::<BTreeMap<_, _>>(),
    )
}

fn audit_url(config: &SnpmConfig) -> String {
    let registry = config.default_registry.trim_end_matches('/');
    format!("{registry}/-/npm/v1/security/audits")
}

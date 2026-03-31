use super::super::filter::is_unfixable;
use super::super::types::{AuditOptions, FixResult, FixedVulnerability, UnfixableVulnerability};
use super::audit::audit;
use crate::{Project, Result, SnpmConfig};

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
            continue;
        }

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

    Ok(FixResult { fixed, unfixable })
}

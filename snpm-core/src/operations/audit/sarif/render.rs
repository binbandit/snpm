use super::super::filter::is_unfixable;
use super::super::types::{AuditResult, Severity};
use super::types::{
    SarifArtifactLocation, SarifDriver, SarifLocation, SarifMessage, SarifPhysicalLocation,
    SarifReport, SarifResult, SarifRule, SarifRuleConfig, SarifRun, SarifTool,
};

const SARIF_SCHEMA: &str = "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json";

impl AuditResult {
    pub fn to_sarif(&self) -> SarifReport {
        let mut rules = Vec::new();
        let mut results = Vec::new();

        for advisory in &self.advisories {
            let rule_id = format!("SNPM-{}", advisory.id);
            let level = sarif_level(advisory.severity);

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
                                fix_text(&advisory.patched_versions),
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

    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::json!({
            "vulnerabilities": self.counts,
            "totalPackages": self.total_packages,
            "advisories": self.advisories,
        })
    }
}

fn sarif_level(severity: Severity) -> &'static str {
    match severity {
        Severity::Critical | Severity::High => "error",
        Severity::Moderate => "warning",
        Severity::Low | Severity::Info => "note",
    }
}

fn fix_text(patched_versions: &str) -> String {
    if is_unfixable(patched_versions) {
        "no fix available".to_string()
    } else {
        patched_versions.to_string()
    }
}

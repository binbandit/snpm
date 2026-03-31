use anyhow::Result;
use snpm_core::operations;

pub(crate) fn print_json(results: &[operations::AuditResult]) -> Result<bool> {
    let mut has_vulnerabilities = false;

    if results.len() == 1 {
        let result = &results[0];
        has_vulnerabilities = !result.advisories.is_empty();
        println!("{}", serde_json::to_string_pretty(&result.to_json_value())?);
    } else {
        let outputs: Vec<_> = results
            .iter()
            .map(|result| {
                if !result.advisories.is_empty() {
                    has_vulnerabilities = true;
                }
                serde_json::json!({
                    "project": result.project_name,
                    "workspaceMember": result.workspace_member,
                    "audit": result.to_json_value(),
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&outputs)?);
    }

    Ok(has_vulnerabilities)
}

pub(crate) fn print_sarif(results: &[operations::AuditResult]) -> Result<bool> {
    let mut has_vulnerabilities = false;
    let mut all_rules = Vec::new();
    let mut all_results = Vec::new();

    for result in results {
        if !result.advisories.is_empty() {
            has_vulnerabilities = true;
        }

        let sarif = result.to_sarif();
        if let Some(run) = sarif.runs.first() {
            all_rules.extend(run.tool.driver.rules.clone());
            all_results.extend(run.results.clone());
        }
    }

    let combined = operations::audit::SarifReport {
        schema: "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/master/Schemata/sarif-schema-2.1.0.json".to_string(),
        version: "2.1.0".to_string(),
        runs: vec![operations::audit::SarifRun {
            tool: operations::audit::SarifTool {
                driver: operations::audit::SarifDriver {
                    name: "snpm-audit".to_string(),
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    information_uri: "https://github.com/nicolo-ribaudo/snpm".to_string(),
                    rules: all_rules,
                },
            },
            results: all_results,
        }],
    };

    println!("{}", serde_json::to_string_pretty(&combined)?);
    Ok(has_vulnerabilities)
}

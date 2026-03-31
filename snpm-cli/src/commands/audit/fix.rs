use anyhow::Result;
use snpm_core::{Project, SnpmConfig, console, operations};

use super::output::render_fix_report;

pub(super) async fn run_fix(
    config: &SnpmConfig,
    cwd: &std::path::Path,
    options: &operations::AuditOptions,
) -> Result<()> {
    let project = Project::discover(cwd)?;
    let result = operations::fix(config, &project, options).await?;

    if result.fixed.is_empty() && result.unfixable.is_empty() {
        console::info("No vulnerabilities found.");
        return Ok(());
    }

    render_fix_report(&result);

    if !result.fixed.is_empty() {
        console::info("Run `snpm install` to apply fixes.");
    }

    Ok(())
}

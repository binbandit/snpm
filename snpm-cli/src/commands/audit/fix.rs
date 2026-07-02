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

    if result.fixable.is_empty() && result.unfixable.is_empty() {
        console::info("No vulnerabilities found.");
        return Ok(());
    }

    render_fix_report(&result);

    if !result.fixable.is_empty() {
        console::info(
            "Update the listed dependencies (e.g. `snpm upgrade <package>` or edit \
             package.json to a patched range) and reinstall to resolve them.",
        );
    }

    Ok(())
}

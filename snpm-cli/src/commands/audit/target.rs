use snpm_core::{Project, SnpmConfig, Workspace, operations};

pub(super) async fn run_audit(
    config: &SnpmConfig,
    cwd: &std::path::Path,
    options: &operations::AuditOptions,
) -> std::result::Result<Vec<operations::AuditResult>, snpm_core::SnpmError> {
    if let Some(workspace) = Workspace::discover(cwd)?
        && workspace.root == *cwd
    {
        operations::audit_workspace(config, &workspace, options).await
    } else {
        let project = Project::discover(cwd)?;
        let result = operations::audit(config, &project, options).await?;
        Ok(vec![result])
    }
}

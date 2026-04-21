use anyhow::{Result, anyhow};
use snpm_core::{Project, Workspace, operations};

use std::path::Path;

pub struct WorkspaceSelection {
    pub projects: Vec<Project>,
    pub filter_label: String,
}

pub fn select_workspace_projects(
    cwd: &Path,
    command: &str,
    recursive: bool,
    filters: &[String],
    filter_prods: &[String],
) -> Result<Option<WorkspaceSelection>> {
    if !recursive && filters.is_empty() && filter_prods.is_empty() {
        return Ok(None);
    }

    let workspace = Workspace::discover(cwd)?.ok_or_else(|| {
        anyhow!("snpm {command}: -r/--filter/--filter-prod used outside a workspace")
    })?;

    let filter_label = operations::format_filters(filters, filter_prods);
    let selected = operations::select_workspace_projects(&workspace, filters, filter_prods)
        .map_err(|err| anyhow!("invalid workspace filter selection for {command}: {err}"))?;
    if selected.is_empty() {
        return Err(anyhow!(
            "snpm {command}: no workspace package matched ({filter_label})"
        ));
    }

    let projects = selected.into_iter().cloned().collect::<Vec<Project>>();

    Ok(Some(WorkspaceSelection {
        projects,
        filter_label,
    }))
}

pub fn project_label(project: &Project) -> String {
    operations::project_label(project)
}

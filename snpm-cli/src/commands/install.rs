use anyhow::{Context, Result};
use clap::Args;
use snpm_core::{Project, SnpmConfig, Workspace, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct InstallArgs {
    /// Skip devDependencies
    #[arg(long)]
    pub production: bool,
    /// Fail if the lockfile is missing or out of date
    #[arg(long = "frozen-lockfile", alias = "immutable")]
    pub frozen_lockfile: bool,
    /// Never reuse a lockfile
    #[arg(
        long = "no-frozen-lockfile",
        conflicts_with_all = ["frozen_lockfile", "prefer_frozen_lockfile"]
    )]
    pub no_frozen_lockfile: bool,
    /// Use the lockfile when compatible, otherwise re-resolve
    #[arg(
        long = "prefer-frozen-lockfile",
        conflicts_with_all = ["frozen_lockfile", "no_frozen_lockfile"]
    )]
    pub prefer_frozen_lockfile: bool,
    /// Re-resolve drifted entries and keep unchanged entries pinned
    #[arg(
        long = "fix-lockfile",
        conflicts_with_all = ["frozen_lockfile", "no_frozen_lockfile", "prefer_frozen_lockfile"]
    )]
    pub fix_lockfile: bool,
    /// Ignore cached state and force a full install
    #[arg(short = 'f', long = "force")]
    pub force: bool,
    /// Packages to install (also updates package.json)
    pub packages: Vec<String>,
    /// Target a specific workspace project by its package name
    #[arg(short = 'w', long = "workspace")]
    pub workspace: Option<String>,
}

pub async fn run(arguments: InstallArgs, config: &SnpmConfig) -> Result<()> {
    console::header("install", env!("CARGO_PKG_VERSION"));

    let current_directory = env::current_dir().context("failed to determine current directory")?;
    let frozen_lockfile = super::frozen::resolve_frozen_lockfile_mode_for_flags(
        config,
        arguments.frozen_lockfile,
        arguments.no_frozen_lockfile,
        arguments.prefer_frozen_lockfile,
        arguments.fix_lockfile,
        arguments.force,
    );

    if let Some(workspace_name) = arguments.workspace {
        let mut workspace = Workspace::discover(&current_directory)?
            .ok_or_else(|| anyhow::anyhow!("snpm install -w used outside a workspace"))?;

        let project = workspace
            .projects
            .iter_mut()
            .find(|project| project.manifest.name.as_deref() == Some(workspace_name.as_str()))
            .ok_or_else(|| anyhow::anyhow!("workspace project {workspace_name} not found"))?;

        let options = operations::InstallOptions {
            requested: arguments.packages,
            dev: false,
            include_dev: !arguments.production,
            frozen_lockfile: frozen_lockfile.mode,
            strict_no_lockfile: frozen_lockfile.strict_no_lockfile,
            force: arguments.force,
            silent_summary: false,
        };
        operations::install(config, project, options).await?;
    } else {
        if arguments.packages.is_empty()
            && let Some(mut workspace) = Workspace::discover(&current_directory)?
            && workspace.root == current_directory
        {
            operations::install_workspace(
                config,
                &mut workspace,
                !arguments.production,
                frozen_lockfile.mode,
                frozen_lockfile.strict_no_lockfile,
                arguments.force,
            )
            .await?;

            return Ok(());
        }

        let mut project = Project::discover(&current_directory)?;
        let options = operations::InstallOptions {
            requested: arguments.packages,
            dev: false,
            include_dev: !arguments.production,
            frozen_lockfile: frozen_lockfile.mode,
            strict_no_lockfile: frozen_lockfile.strict_no_lockfile,
            force: arguments.force,
            silent_summary: false,
        };
        operations::install(config, &mut project, options).await?;
    }

    Ok(())
}

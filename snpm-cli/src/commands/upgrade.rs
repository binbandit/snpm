use anyhow::Result;
use clap::Args;
use snpm_core::{Project, SnpmConfig, Workspace, console, operations};
use std::env;
use std::fs;

#[derive(Args, Debug)]
pub struct UpgradeArgs {
    #[arg(long)]
    pub production: bool,
    #[arg(short = 'f', long = "force")]
    pub force: bool,
    pub packages: Vec<String>,
}

pub async fn run(args: UpgradeArgs, config: &SnpmConfig) -> Result<()> {
    console::header("upgrade", env!("CARGO_PKG_VERSION"));

    let cwd = env::current_dir()?;

    if !args.packages.is_empty() {
        let mut project = Project::discover(&cwd)?;
        operations::upgrade(
            config,
            &mut project,
            args.packages,
            args.production,
            args.force,
        )
        .await?;
        return Ok(());
    }

    let mut project = Project::discover(&cwd)?;
    let workspace = Workspace::discover(&cwd)?;

    let lockfile_path = workspace
        .as_ref()
        .map(|w| w.root.join("snpm-lock.yaml"))
        .unwrap_or_else(|| project.root.join("snpm-lock.yaml"));

    if lockfile_path.is_file() {
        fs::remove_file(&lockfile_path)?;
    }

    if let Some(mut workspace) = workspace {
        if workspace.root == cwd {
            operations::install_workspace(
                config,
                &mut workspace,
                !args.production,
                false,
                args.force,
            )
            .await?;

            return Ok(());
        }
    }

    let options = operations::InstallOptions {
        requested: Vec::new(),
        dev: false,
        include_dev: !args.production,
        frozen_lockfile: false,
        force: args.force,
        silent_summary: false,
    };

    operations::install(config, &mut project, options).await?;

    Ok(())
}

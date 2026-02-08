use anyhow::Result;
use clap::Args;
use snpm_core::{OfflineMode, SnpmConfig, console, operations};
use std::env;

#[derive(Args, Debug)]
pub struct DlxArgs {
    /// Package to download and run (e.g. "cowsay" or "cowsay@latest")
    pub package: String,

    /// Run without network access; fail if package is not cached
    #[arg(long)]
    pub offline: bool,

    /// Prefer cached packages; only fetch if not in cache
    #[arg(long = "prefer-offline")]
    pub prefer_offline: bool,

    /// Arguments to pass to the package's binary
    #[arg(trailing_var_arg = true)]
    pub args: Vec<String>,
}

pub async fn run(args: DlxArgs, config: &SnpmConfig) -> Result<()> {
    console::header(&format!("dlx {}", args.package), env!("CARGO_PKG_VERSION"));

    let offline_mode = if args.offline {
        OfflineMode::Offline
    } else if args.prefer_offline {
        OfflineMode::PreferOffline
    } else {
        OfflineMode::Online
    };

    operations::dlx_with_offline(config, args.package, args.args, offline_mode).await?;
    Ok(())
}

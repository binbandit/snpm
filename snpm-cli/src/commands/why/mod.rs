mod render;
mod target;

use anyhow::{Context, Result};
use clap::Args;
use snpm_core::console;
use std::env;

#[derive(Args, Debug)]
pub struct WhyArgs {
    /// Package name or pattern (supports `*`)
    pub package: String,

    /// Maximum reverse dependency depth
    #[arg(long)]
    pub depth: Option<usize>,

    /// Output JSON
    #[arg(long)]
    pub json: bool,
}

pub async fn run(args: WhyArgs) -> Result<()> {
    if !args.json {
        console::header("why", env!("CARGO_PKG_VERSION"));
    }

    let cwd = env::current_dir().context("failed to determine current directory")?;

    if target::try_run_workspace(&cwd, &args)? {
        return Ok(());
    }

    let result = target::run_project(&cwd, &args)?;
    if result.matches.is_empty() {
        render::print_no_results(&args.package);
        return Ok(());
    }

    render::print_result(&result, args.json)
}

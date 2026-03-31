mod format;
mod print;

use anyhow::Result;
use clap::Args;
use snpm_core::{SnpmConfig, console};

#[derive(Args, Debug)]
pub struct ConfigArgs {}

pub async fn run(_args: ConfigArgs, config: &SnpmConfig) -> Result<()> {
    console::header("config", env!("CARGO_PKG_VERSION"));

    print::print_paths(config);
    print::print_registry(config);
    print::print_install(config);
    print::print_scripts(config);
    print::print_logging(config);

    Ok(())
}

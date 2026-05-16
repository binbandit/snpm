use anyhow::Result;
use clap::Args;
use snpm_core::SnpmConfig;
use snpm_core::node::aliases;

#[derive(Args, Debug)]
pub struct UnaliasArgs {
    /// Alias name to remove
    pub name: String,
}

pub fn run(args: UnaliasArgs, config: &SnpmConfig) -> Result<()> {
    let removed = aliases::remove_alias(config, &args.name)?;
    if removed {
        println!("Removed alias '{}'", args.name);
    } else {
        println!("Alias '{}' did not exist", args.name);
    }
    Ok(())
}

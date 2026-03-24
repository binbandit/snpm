use anyhow::Result;
use clap::CommandFactory;
use clap_complete::{Shell, generate};

#[derive(clap::Args, Debug)]
pub struct CompletionsArgs {
    /// Shell to generate completions for (bash, zsh, fish, powershell, elvish)
    pub shell: Shell,
}

pub async fn run(args: CompletionsArgs) -> Result<()> {
    let mut cmd = crate::cli::Cli::command();
    generate(args.shell, &mut cmd, "snpm", &mut std::io::stdout());
    Ok(())
}

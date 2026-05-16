use anyhow::Result;
use clap::Args;
use snpm_core::node::shell::{ShellFlavor, shell_init_script};

#[derive(Args, Debug)]
pub struct EnvArgs {
    /// Shell flavor (bash | zsh | fish | powershell). Defaults to $SHELL.
    #[arg(long = "shell")]
    pub shell: Option<String>,
}

pub fn run(args: EnvArgs) -> Result<()> {
    let flavor = match args.shell {
        Some(name) => ShellFlavor::parse(&name)?,
        None => ShellFlavor::detect()?,
    };
    print!("{}", shell_init_script(flavor));
    Ok(())
}

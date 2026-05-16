use anyhow::Result;
use clap::Args;
use snpm_core::SnpmConfig;
use snpm_core::node::exec as node_exec;

use std::env;
use std::process::Command;

#[derive(Args, Debug)]
pub struct ExecArgs {
    /// Node.js version to run the command under (`20`, `lts`, alias name, etc.)
    pub version: String,
    /// Command to execute (defaults to `node` if omitted)
    pub command: Option<String>,
    /// Arguments forwarded to the command
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

pub async fn run(args: ExecArgs, config: &SnpmConfig) -> Result<()> {
    let (_, summary) = node_exec::ensure_installed_for_spec(config, &args.version).await?;
    let bin_dir = config.node_version_bin_dir(&summary.version);

    let command = args.command.unwrap_or_else(|| "node".to_string());
    let mut process = Command::new(command_path(&command, &bin_dir));
    process.args(&args.args);

    let path_value = prepend_bin_to_path(&bin_dir)?;
    process.env("PATH", path_value);

    let status = process.status()?;
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    Ok(())
}

fn command_path(command: &str, bin_dir: &std::path::Path) -> std::path::PathBuf {
    if command.contains(std::path::MAIN_SEPARATOR) {
        return std::path::PathBuf::from(command);
    }
    let candidate = bin_dir.join(command);
    if candidate.exists() {
        candidate
    } else {
        std::path::PathBuf::from(command)
    }
}

fn prepend_bin_to_path(bin_dir: &std::path::Path) -> Result<std::ffi::OsString> {
    let mut parts = vec![bin_dir.to_path_buf()];
    if let Some(existing) = env::var_os("PATH") {
        for path in env::split_paths(&existing) {
            parts.push(path);
        }
    }
    Ok(env::join_paths(parts)?)
}

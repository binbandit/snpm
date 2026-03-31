mod auth;
mod prompt;

use anyhow::Result;
use clap::{Args, ValueEnum};
use snpm_core::{SnpmConfig, console, operations};
use std::env;

use auth::{authenticate, extract_host};

#[derive(Debug, Clone, Copy, ValueEnum, Default)]
pub enum AuthTypeArg {
    #[default]
    Web,
    Legacy,
}

impl From<AuthTypeArg> for operations::AuthType {
    fn from(arg: AuthTypeArg) -> Self {
        match arg {
            AuthTypeArg::Web => operations::AuthType::Web,
            AuthTypeArg::Legacy => operations::AuthType::Legacy,
        }
    }
}

#[derive(Args, Debug)]
pub struct LoginArgs {
    #[arg(long, help = "Registry URL (defaults to configured registry)")]
    pub registry: Option<String>,

    #[arg(long, value_enum, default_value = "web", hide = true)]
    pub auth_type: AuthTypeArg,

    #[arg(long, help = "Associate credentials with a scope (e.g. @myorg)")]
    pub scope: Option<String>,
}

pub async fn run(args: LoginArgs, config: &SnpmConfig) -> Result<()> {
    console::header("login", env!("CARGO_PKG_VERSION"));

    let registry = args
        .registry
        .as_deref()
        .unwrap_or(config.default_registry.as_str());

    let host = extract_host(registry);
    console::step(&format!("Logging in to {host}..."));

    let auth_result = authenticate(registry, args.auth_type.into()).await?;

    operations::save_credentials(
        config,
        args.registry.as_deref(),
        &auth_result.token,
        args.scope.as_deref(),
    )?;

    println!();
    let user = auth_result
        .username
        .map(|username| format!(" as {username}"))
        .unwrap_or_default();
    let scope = args
        .scope
        .as_ref()
        .map(|scope| format!(" for scope {scope}"))
        .unwrap_or_default();
    console::info(&format!("Logged in{user}{scope} on {host}"));

    Ok(())
}

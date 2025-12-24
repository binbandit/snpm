use anyhow::{Result, anyhow};
use clap::Args;
use snpm_core::{SnpmConfig, console, operations};
use std::env;
use std::io::{self, Write};

#[derive(Args, Debug)]
pub struct LoginArgs {
    /// Registry URL to store credentials for. Defaults to the current default registry.
    #[arg(long)]
    pub registry: Option<String>,
    /// Auth token to save. If omitted, snpm will prompt for it.
    #[arg(long)]
    pub token: Option<String>,
    /// Associate a scope with the registry (e.g., @myorg)
    #[arg(long)]
    pub scope: Option<String>,
    /// Open browser for web-based authentication
    #[arg(long)]
    pub web: bool,
}

pub async fn run(args: LoginArgs, config: &SnpmConfig) -> Result<()> {
    console::header("login", env!("CARGO_PKG_VERSION"));

    let registry_url = args
        .registry
        .as_deref()
        .unwrap_or(config.default_registry.as_str());

    let token_value = if let Some(t) = args.token {
        t.trim().to_string()
    } else if args.web {
        web_login(registry_url, args.scope.as_deref()).await?
    } else {
        prompt_for_token(registry_url)?
    };

    if token_value.is_empty() {
        return Err(anyhow!("auth token cannot be empty"));
    }

    operations::login(
        config,
        args.registry.as_deref(),
        &token_value,
        args.scope.as_deref(),
    )?;

    console::info("Credentials saved successfully");
    Ok(())
}

fn prompt_for_token(registry_url: &str) -> Result<String> {
    console::info(&format!("Logging in to {registry_url}"));
    console::info("To create a token, visit:");

    let token_url = if registry_url.contains("registry.npmjs.org") {
        "https://www.npmjs.com/settings/tokens/new"
    } else {
        registry_url
    };

    console::info(&format!("  {token_url}"));
    println!();

    print!("Enter your auth token: ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    Ok(input.trim().to_string())
}

async fn web_login(registry_url: &str, _scope: Option<&str>) -> Result<String> {
    use reqwest::Client;
    use serde::Deserialize;
    use std::time::Duration;

    #[derive(Deserialize)]
    struct LoginResponse {
        #[serde(rename = "loginUrl")]
        login_url: String,
        #[serde(rename = "doneUrl")]
        done_url: String,
    }

    #[derive(Deserialize)]
    struct TokenResponse {
        token: Option<String>,
    }

    let client = Client::new();
    let endpoint = format!("{}/-/v1/login", registry_url.trim_end_matches('/'));

    console::info("Initiating web-based login...");

    let response = client
        .post(&endpoint)
        .json(&serde_json::json!({}))
        .send()
        .await?;

    if !response.status().is_success() {
        if response.status().as_u16() >= 400 && response.status().as_u16() < 500 {
            return Err(anyhow!("Web login not supported. Use --token instead."));
        }
        return Err(anyhow!("Login failed: {}", response.status()));
    }

    let urls: LoginResponse = response.json().await?;

    console::info("Opening browser for authentication...");
    console::info(&format!("  {}", urls.login_url));

    if let Err(e) = open::that(&urls.login_url) {
        console::info(&format!("Failed to open browser: {e}"));
        console::info("Please manually visit the URL above");
    }

    println!();
    console::info("Waiting for authentication...");

    for attempt in 1..=120 {
        tokio::time::sleep(Duration::from_secs(2)).await;

        let response = client.get(&urls.done_url).send().await?;
        let status = response.status().as_u16();

        match status {
            200 => {
                let data: TokenResponse = response.json().await?;
                let token = data.token.ok_or_else(|| anyhow!("No token received"))?;
                console::info("Authentication successful!");
                return Ok(token);
            }
            202 => {
                if attempt % 5 == 0 {
                    console::info("Still waiting...");
                }
            }
            _ => return Err(anyhow!("Authentication failed: {status}")),
        }
    }

    Err(anyhow!("Authentication timeout"))
}

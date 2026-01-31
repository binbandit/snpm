use anyhow::{Result, anyhow};
use clap::{Args, ValueEnum};
use snpm_core::{SnpmConfig, SnpmError, console, operations};
use std::env;
use std::io::{self, Write};

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
        .map(|u| format!(" as {u}"))
        .unwrap_or_default();
    let scope = args
        .scope
        .as_ref()
        .map(|s| format!(" for scope {s}"))
        .unwrap_or_default();
    console::info(&format!("Logged in{user}{scope} on {host}"));

    Ok(())
}

async fn authenticate(
    registry: &str,
    auth_type: operations::AuthType,
) -> Result<operations::AuthResult> {
    let mut cached_credentials: Option<operations::Credentials> = None;
    let mut otp: Option<String> = None;

    loop {
        let opener = create_opener();

        let credentials = cached_credentials.clone();
        let prompt = move || -> snpm_core::Result<operations::Credentials> {
            if let Some(creds) = credentials {
                return Ok(creds);
            }

            let username = read_line("Username: ").map_err(|e| SnpmError::Auth {
                reason: e.to_string(),
            })?;
            let password = read_password("Password: ").map_err(|e| SnpmError::Auth {
                reason: e.to_string(),
            })?;

            Ok(operations::Credentials {
                username: Some(username),
                password: Some(password),
            })
        };

        match operations::login_with_fallback(registry, auth_type, opener, prompt, otp.as_deref())
            .await
        {
            Ok(result) => return Ok(result),

            Err(SnpmError::Auth { reason }) if reason == "OTP required" => {
                if cached_credentials.is_none() {
                    cached_credentials = Some(operations::Credentials {
                        username: Some(read_line("Username: ")?),
                        password: Some(read_password("Password: ")?),
                    });
                }
                otp = Some(prompt_otp()?);
            }

            Err(e) => return Err(e.into()),
        }
    }
}

fn create_opener() -> operations::OpenerFn {
    Box::new(|url: &str| {
        println!();
        print!("Press ENTER to open browser... ");
        let _ = io::stdout().flush();

        let mut input = String::new();
        let _ = io::stdin().read_line(&mut input);

        open::that(url).map_err(|e| e.to_string())
    })
}

fn extract_host(url: &str) -> &str {
    url.trim_start_matches("https://")
        .trim_start_matches("http://")
        .split('/')
        .next()
        .unwrap_or(url)
}

fn read_line(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;

    let value = input.trim().to_string();
    if value.is_empty() {
        return Err(anyhow!("{} is required", prompt.trim_end_matches(':')));
    }

    Ok(value)
}

fn read_password(prompt: &str) -> Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;

    let password = if atty::is(atty::Stream::Stdin) {
        rpassword::read_password()?
    } else {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        input.trim().to_string()
    };

    if password.is_empty() {
        return Err(anyhow!("Password is required"));
    }

    Ok(password)
}

fn prompt_otp() -> Result<String> {
    println!();
    let otp = read_line("One-time password: ")?;
    Ok(otp.replace(' ', ""))
}

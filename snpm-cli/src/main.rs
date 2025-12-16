use anyhow::{Result, anyhow};
use clap::Parser;
use snpm_core::{
    Project, SnpmConfig, Workspace, console,
    operations::{self, InstallOptions},
};
use std::{
    env, fs,
    io::{self, Write},
    process,
};
use tracing_subscriber::EnvFilter;

mod cli;

use cli::{Cli, Command};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        console::error(&format!("{error}"));
        process::exit(1);
    }
}

async fn run() -> Result<()> {
    let Cli { verbose, command } = Cli::parse();

    init_tracing()?;

    let mut config = SnpmConfig::from_env();

    if verbose || config.verbose || config.log_file.is_some() {
        config.verbose = true;

        let cwd = env::current_dir()?;
        let log_path = config
            .log_file
            .clone()
            .unwrap_or_else(|| cwd.join(".snpm.log"));

        console::init_logging(&log_path)?;
        console::verbose(&format!(
            "verbose logging enabled, writing to {}",
            log_path.display()
        ));
    }

    match command {
        Command::Install {
            packages,
            production,
            frozen_lockfile,
            force,
            workspace: target,
        } => {
            console::header("install");

            let cwd = env::current_dir()?;

            if let Some(workspace_name) = target {
                let mut workspace = Workspace::discover(&cwd)?
                    .ok_or_else(|| anyhow::anyhow!("snpm install -w used outside a workspace"))?;

                let project = workspace
                    .projects
                    .iter_mut()
                    .find(|p| p.manifest.name.as_deref() == Some(workspace_name.as_str()))
                    .ok_or_else(|| {
                        anyhow::anyhow!(format!("workspace project {workspace_name} not found"))
                    })?;

                let options = operations::InstallOptions {
                    requested: packages,
                    dev: false,
                    include_dev: !production,
                    frozen_lockfile,
                    force,
                    silent_summary: false,
                };
                operations::install(&config, project, options).await?;
            } else {
                if packages.is_empty() {
                    if let Some(mut workspace) = Workspace::discover(&cwd)? {
                        if workspace.root == cwd {
                            operations::install_workspace(
                                &config,
                                &mut workspace,
                                !production,
                                frozen_lockfile,
                                force,
                            )
                            .await?;

                            return Ok(());
                        }
                    }
                }

                let mut project = Project::discover(&cwd)?;
                let options = operations::InstallOptions {
                    requested: packages,
                    dev: false,
                    include_dev: !production,
                    frozen_lockfile,
                    force,
                    silent_summary: false,
                };
                operations::install(&config, &mut project, options).await?;
            }
        }

        Command::Add {
            dev,
            workspace: target,
            packages,
            force,
        } => {
            console::header("add");

            let cwd = env::current_dir()?;

            if let Some(workspace_name) = target {
                let mut workspace = Workspace::discover(&cwd)?
                    .ok_or_else(|| anyhow::anyhow!("snpm add -w used outside a workspace"))?;

                let project = workspace
                    .projects
                    .iter_mut()
                    .find(|p| p.manifest.name.as_deref() == Some(workspace_name.as_str()))
                    .ok_or_else(|| {
                        anyhow::anyhow!(format!("workspace project {workspace_name} not found"))
                    })?;

                let options = operations::InstallOptions {
                    requested: packages,
                    dev,
                    include_dev: true,
                    frozen_lockfile: false,
                    force,
                    silent_summary: false,
                };
                operations::install(&config, project, options).await?;
            } else {
                let mut project = Project::discover(&cwd)?;
                let options = InstallOptions {
                    requested: packages,
                    dev,
                    include_dev: true,
                    frozen_lockfile: false,
                    force,
                    silent_summary: false,
                };
                operations::install(&config, &mut project, options).await?;
            }
        }

        Command::Remove { packages } => {
            console::header("remove");

            let cwd = env::current_dir()?;
            let mut project = Project::discover(&cwd)?;
            operations::remove(&config, &mut project, packages).await?;
        }

        Command::Dlx { package, args } => {
            console::header(&format!("dlx {}", package));
            operations::dlx(&config, package, args).await?;
        }

        Command::Run {
            script,
            args,
            recursive,
            filter,
        } => {
            console::header(&format!("run {}", script));

            let cwd = env::current_dir()?;

            if recursive || !filter.is_empty() {
                let workspace = Workspace::discover(&cwd)?
                    .ok_or_else(|| anyhow!("snpm run -r/--filter used outside a workspace"))?;

                operations::run_workspace_scripts(&workspace, &script, &filter, &args)?;
            } else {
                let project = Project::discover(&cwd)?;
                operations::run_script(&project, &script, &args)?;
            }
        }

        Command::Init => {
            console::header("init");

            let cwd = env::current_dir()?;
            operations::init(&cwd)?;
        }

        Command::Upgrade {
            production,
            force,
            packages,
        } => {
            console::header("upgrade");

            let cwd = env::current_dir()?;

            if !packages.is_empty() {
                let mut project = Project::discover(&cwd)?;
                operations::upgrade(&config, &mut project, packages, production, force).await?;
                return Ok(());
            }

            let mut project = Project::discover(&cwd)?;
            let workspace = Workspace::discover(&cwd)?;

            let lockfile_path = workspace
                .as_ref()
                .map(|w| w.root.join("snpm-lock.yaml"))
                .unwrap_or_else(|| project.root.join("snpm-lock.yaml"));

            if lockfile_path.is_file() {
                fs::remove_file(&lockfile_path)?;
            }

            if let Some(mut workspace) = workspace {
                if workspace.root == cwd {
                    operations::install_workspace(
                        &config,
                        &mut workspace,
                        !production,
                        false,
                        force,
                    )
                    .await?;

                    return Ok(());
                }
            }

            let options = InstallOptions {
                requested: Vec::new(),
                dev: false,
                include_dev: !production,
                frozen_lockfile: false,
                force,
                silent_summary: false,
            };

            operations::install(&config, &mut project, options).await?;
        }

        Command::Outdated { production } => {
            console::header("outdated");

            let cwd = env::current_dir()?;

            if let Some(workspace) = Workspace::discover(&cwd)? {
                if workspace.root == cwd {
                    let mut any = false;

                    for project in workspace.projects.iter() {
                        let entries =
                            operations::outdated(&config, project, !production, false).await?;

                        if entries.is_empty() {
                            continue;
                        }

                        if any {
                            println!();
                        }
                        any = true;

                        let name = project_label(project);
                        println!("\n{}", name);
                        print_outdated(&entries);
                    }

                    if !any {
                        console::info("All dependencies are up to date.");
                    }

                    return Ok(());
                }
            }

            let project = Project::discover(&cwd)?;
            let entries = operations::outdated(&config, &project, !production, false).await?;

            if entries.is_empty() {
                console::info("All dependencies are up to date.");
            } else {
                let name = project_label(&project);
                println!("\n{}", name);
                print_outdated(&entries);
            }
        }
        Command::Login {
            registry,
            token,
            scope,
            web,
        } => {
            console::header("login");

            let registry_url = registry
                .as_deref()
                .unwrap_or(config.default_registry.as_str());

            let token_value = if let Some(t) = token {
                t.trim().to_string()
            } else if web {
                web_login(registry_url, scope.as_deref()).await?
            } else {
                prompt_for_token(registry_url)?
            };

            if token_value.is_empty() {
                return Err(anyhow!("auth token cannot be empty"));
            }

            operations::login(&config, registry.as_deref(), &token_value, scope.as_deref())?;

            console::info("Credentials saved successfully");
        }

        Command::Logout { registry, scope } => {
            console::header("logout");

            operations::logout(&config, registry.as_deref(), scope.as_deref())?;

            console::info("Credentials removed successfully");
        }
    }

    Ok(())
}

fn init_tracing() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();
    Ok(())
}

fn print_outdated(entries: &[operations::OutdatedEntry]) {
    if entries.is_empty() {
        return;
    }

    let mut name_width = 4;

    for entry in entries {
        if entry.name.len() > name_width {
            name_width = entry.name.len();
        }
    }

    let header_name = "name";
    let header_current = "current";
    let header_wanted = "wanted";

    println!(
        "{:<name_width$}  {:<10}  {:<10}",
        header_name,
        header_current,
        header_wanted,
        name_width = name_width
    );

    for entry in entries {
        let current = entry.current.as_deref().unwrap_or("-");
        println!(
            "{:<name_width$}  {:<10}  {:<10}",
            entry.name,
            current,
            entry.wanted,
            name_width = name_width
        );
    }
}

fn project_label(project: &Project) -> String {
    if let Some(name) = project.manifest.name.as_deref() {
        name.to_string()
    } else {
        project
            .root
            .file_name()
            .and_then(|os| os.to_str())
            .unwrap_or(".")
            .to_string()
    }
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

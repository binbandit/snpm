mod config;
mod manifest;
mod version;

use std::env;
use std::process::{Command, ExitCode, Stdio};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();

    match run(args) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("snpm-switch: {}", error);
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> anyhow::Result<ExitCode> {
    if args.first().map(|s| s.as_str()) == Some("switch") {
        return handle_switch_command(&args[1..]);
    }

    let package_manager = manifest::find_package_manager()?;

    let snpm_binary = match package_manager {
        Some(reference) => {
            if !reference.name.eq_ignore_ascii_case("snpm") {
                anyhow::bail!(
                    "packageManager field specifies '{}', not snpm",
                    reference.name
                );
            }

            version::ensure_version(&reference.version)?
        }
        None => version::ensure_latest()?,
    };

    let mut command = Command::new(&snpm_binary);
    command.args(&args);
    command.stdin(Stdio::inherit());
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());

    let status = command.status()?;

    Ok(ExitCode::from(status.code().unwrap_or(1) as u8))
}

fn handle_switch_command(args: &[String]) -> anyhow::Result<ExitCode> {
    match args.first().map(|s| s.as_str()) {
        Some("list") => {
            let versions = version::list_cached_versions()?;

            if versions.is_empty() {
                println!("No snpm versions cached.");
            } else {
                println!("Cached snpm versions:");
                for v in versions {
                    println!("  {}", v);
                }
            }

            Ok(ExitCode::SUCCESS)
        }

        Some("cache") => {
            if args.get(1).map(|s| s.as_str()) == Some("--install") {
                let package_manager = manifest::find_package_manager()?;

                match package_manager {
                    Some(reference) => {
                        println!("Caching snpm {}...", reference.version);
                        version::ensure_version(&reference.version)?;
                        println!("Done.");
                    }
                    None => {
                        println!("No packageManager field found, caching latest...");
                        version::ensure_latest()?;
                        println!("Done.");
                    }
                }
            }

            Ok(ExitCode::SUCCESS)
        }

        Some("clear") => {
            version::clear_cache()?;
            println!("Cache cleared.");
            Ok(ExitCode::SUCCESS)
        }

        Some(subcommand) => {
            anyhow::bail!("Unknown switch subcommand: {}", subcommand);
        }

        None => {
            println!("snpm-switch - Version manager for snpm\n");
            println!("Usage:");
            println!("  snpm-switch <snpm-command>  Run snpm with the version from packageManager");
            println!("  snpm-switch switch list    List cached snpm versions");
            println!("  snpm-switch switch cache   Cache the version from packageManager");
            println!("  snpm-switch switch clear   Clear the version cache");
            Ok(ExitCode::SUCCESS)
        }
    }
}

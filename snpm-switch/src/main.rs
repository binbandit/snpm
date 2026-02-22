mod config;
mod manifest;
mod version;

use crate::manifest::{PackageManagerReference, PackageManagerSpecifier};
use std::env;
use std::path::PathBuf;
use std::process::{Command, ExitCode, ExitStatus, Stdio};

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

    let snpm_binary = resolve_binary(None)?;

    let mut command = Command::new(&snpm_binary);
    command.args(&args);
    command.stdin(Stdio::inherit());
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());

    let status = command.status()?;
    Ok(exit_code_from_status(status))
}

fn exit_code_from_status(status: ExitStatus) -> ExitCode {
    status
        .code()
        .and_then(|code| u8::try_from(code).ok())
        .map(ExitCode::from)
        .unwrap_or_else(|| ExitCode::from(1))
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
            let mut requested_versions = Vec::new();

            for arg in &args[1..] {
                if arg == "--install" {
                    continue;
                }

                if arg.starts_with('-') {
                    anyhow::bail!("Unknown switch cache flag: {}", arg);
                }

                requested_versions.push(arg.as_str());
            }

            if requested_versions.is_empty() {
                cache_from_project_or_latest()?;
            } else {
                for requested_version in requested_versions {
                    println!("Caching snpm {}...", requested_version);
                    version::ensure_version(requested_version)?;
                    println!("Done.");
                }
            }

            Ok(ExitCode::SUCCESS)
        }

        Some("which") => {
            let path = resolve_binary(None)?;
            println!("{}", path.display());
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
            println!(
                "  snpm-switch switch cache   Cache project/default version or explicit versions"
            );
            println!("  snpm-switch switch which   Print the binary path that would be executed");
            println!("  snpm-switch switch clear   Clear the version cache");
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn cache_from_project_or_latest() -> anyhow::Result<()> {
    match manifest::find_package_manager()? {
        Some(reference) => {
            ensure_is_snpm(&reference)?;

            match reference.reference {
                PackageManagerSpecifier::Version(version) => {
                    println!("Caching snpm {}...", version);
                    version::ensure_version(&version)?;
                    println!("Done.");
                }
                PackageManagerSpecifier::Local(path) => {
                    if !path.is_file() {
                        anyhow::bail!(
                            "packageManager points to local binary '{}' but it does not exist",
                            path.display()
                        );
                    }

                    println!(
                        "packageManager points to local binary '{}'; nothing to cache.",
                        path.display()
                    );
                }
            }
        }
        None => {
            println!("No packageManager field found, caching latest...");
            version::ensure_latest()?;
            println!("Done.");
        }
    }

    Ok(())
}

fn resolve_binary(reference: Option<PackageManagerReference>) -> anyhow::Result<PathBuf> {
    let reference = match reference {
        Some(reference) => Some(reference),
        None => manifest::find_package_manager()?,
    };

    match reference {
        Some(reference) => {
            ensure_is_snpm(&reference)?;

            match reference.reference {
                PackageManagerSpecifier::Version(version) => version::ensure_version(&version),
                PackageManagerSpecifier::Local(path) => {
                    if path.is_file() {
                        Ok(path)
                    } else {
                        anyhow::bail!(
                            "packageManager points to local binary '{}' but it does not exist",
                            path.display()
                        )
                    }
                }
            }
        }
        None => {
            if let Some(path) = bundled_binary_path() {
                Ok(path)
            } else {
                version::ensure_latest()
            }
        }
    }
}

fn ensure_is_snpm(reference: &PackageManagerReference) -> anyhow::Result<()> {
    if reference.name.eq_ignore_ascii_case("snpm") {
        Ok(())
    } else {
        anyhow::bail!(
            "packageManager field specifies '{}', not snpm",
            reference.name
        )
    }
}

fn bundled_binary_path() -> Option<PathBuf> {
    let current = std::env::current_exe().ok()?;
    let binary_name = if cfg!(windows) { "snpm.exe" } else { "snpm" };
    let candidate = current.parent()?.join(binary_name);

    if candidate.is_file() {
        Some(candidate)
    } else {
        None
    }
}

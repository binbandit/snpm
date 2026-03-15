mod config;
mod manifest;
mod version;

use crate::manifest::{PackageManagerReference, PackageManagerSpecifier};
use std::env;
use std::path::PathBuf;
use std::process::{Command, ExitCode, ExitStatus, Stdio};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct SwitchOptions {
    ignore_package_manager: bool,
    version_override: Option<String>,
}

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
    let (options, args) = parse_switch_options(args)?;

    if args.first().map(|s| s.as_str()) == Some("switch") {
        return handle_switch_command(&args[1..], &options);
    }

    let snpm_binary = resolve_binary(None, &options)?;

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

fn parse_switch_options(args: Vec<String>) -> anyhow::Result<(SwitchOptions, Vec<String>)> {
    let mut options = SwitchOptions::default();
    let mut remaining = Vec::new();
    let mut index = 0;

    while index < args.len() {
        let arg = &args[index];

        if arg == "--" {
            remaining.extend(args[index..].iter().cloned());
            break;
        }

        if arg == "--switch-ignore-package-manager" {
            set_ignore_package_manager(&mut options)?;
            index += 1;
            continue;
        }

        if let Some(value) = arg.strip_prefix("--switch-version=") {
            set_version_override(&mut options, value)?;
            index += 1;
            continue;
        }

        if arg == "--switch-version" {
            let value = args
                .get(index + 1)
                .ok_or_else(|| anyhow::anyhow!("--switch-version requires a version argument"))?;

            set_version_override(&mut options, value)?;
            index += 2;
            continue;
        }

        remaining.push(arg.clone());
        index += 1;
    }

    Ok((options, remaining))
}

fn set_ignore_package_manager(options: &mut SwitchOptions) -> anyhow::Result<()> {
    if options.version_override.is_some() {
        anyhow::bail!("Cannot combine --switch-ignore-package-manager with --switch-version");
    }

    options.ignore_package_manager = true;
    Ok(())
}

fn set_version_override(options: &mut SwitchOptions, value: &str) -> anyhow::Result<()> {
    if options.ignore_package_manager {
        anyhow::bail!("Cannot combine --switch-version with --switch-ignore-package-manager");
    }

    if value.is_empty() {
        anyhow::bail!("--switch-version requires a non-empty version");
    }

    match options.version_override.as_deref() {
        Some(existing) if existing == value => Ok(()),
        Some(existing) => anyhow::bail!(
            "Conflicting --switch-version values: '{}' and '{}'",
            existing,
            value
        ),
        None => {
            options.version_override = Some(value.to_string());
            Ok(())
        }
    }
}

fn handle_switch_command(args: &[String], options: &SwitchOptions) -> anyhow::Result<ExitCode> {
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
                cache_from_project_or_latest(options)?;
            } else {
                if options.version_override.is_some() {
                    anyhow::bail!(
                        "Cannot combine --switch-version with explicit versions for 'switch cache'"
                    );
                }

                for requested_version in requested_versions {
                    println!("Caching snpm {}...", requested_version);
                    version::ensure_version(requested_version)?;
                    println!("Done.");
                }
            }

            Ok(ExitCode::SUCCESS)
        }

        Some("which") => {
            let path = resolve_binary(None, options)?;
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
            println!(
                "  snpm-switch [--switch-version <version> | --switch-ignore-package-manager] <snpm-command>"
            );
            println!("  snpm-switch switch list    List cached snpm versions");
            println!(
                "  snpm-switch switch cache   Cache project/default version or explicit versions"
            );
            println!("  snpm-switch switch which   Print the binary path that would be executed");
            println!("  snpm-switch switch clear   Clear the version cache");
            println!();
            println!("Switch flags:");
            println!("  --switch-version <version>         Override the project packageManager");
            println!("  --switch-ignore-package-manager    Ignore the project packageManager");
            Ok(ExitCode::SUCCESS)
        }
    }
}

fn cache_from_project_or_latest(options: &SwitchOptions) -> anyhow::Result<()> {
    let project_reference = manifest::find_package_manager()?;
    let ignored_project_package_manager =
        options.ignore_package_manager && project_reference.is_some();
    let reference = select_package_manager_reference(project_reference, options);

    match reference {
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
            if ignored_project_package_manager {
                println!("Ignoring packageManager field, caching latest...");
            } else {
                println!("No packageManager field found, caching latest...");
            }
            version::ensure_latest()?;
            println!("Done.");
        }
    }

    Ok(())
}

fn resolve_binary(
    reference: Option<PackageManagerReference>,
    options: &SwitchOptions,
) -> anyhow::Result<PathBuf> {
    let reference = match reference {
        Some(reference) => Some(reference),
        None => manifest::find_package_manager()?,
    };

    match select_package_manager_reference(reference, options) {
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

fn select_package_manager_reference(
    reference: Option<PackageManagerReference>,
    options: &SwitchOptions,
) -> Option<PackageManagerReference> {
    if let Some(version) = &options.version_override {
        return Some(PackageManagerReference {
            name: "snpm".to_string(),
            reference: PackageManagerSpecifier::Version(version.clone()),
        });
    }

    if options.ignore_package_manager {
        return None;
    }

    reference
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

#[cfg(test)]
mod tests {
    use super::*;

    fn version_reference(name: &str, version: &str) -> PackageManagerReference {
        PackageManagerReference {
            name: name.to_string(),
            reference: PackageManagerSpecifier::Version(version.to_string()),
        }
    }

    #[test]
    fn parse_switch_options_strips_switch_flags_anywhere() {
        let (options, args) = parse_switch_options(vec![
            "install".to_string(),
            "--switch-ignore-package-manager".to_string(),
            "--filter".to_string(),
            "pkg".to_string(),
        ])
        .unwrap();

        assert_eq!(
            options,
            SwitchOptions {
                ignore_package_manager: true,
                version_override: None,
            }
        );
        assert_eq!(
            args,
            vec![
                "install".to_string(),
                "--filter".to_string(),
                "pkg".to_string(),
            ]
        );
    }

    #[test]
    fn parse_switch_options_stops_at_double_dash() {
        let (options, args) = parse_switch_options(vec![
            "install".to_string(),
            "--".to_string(),
            "--switch-ignore-package-manager".to_string(),
        ])
        .unwrap();

        assert_eq!(options, SwitchOptions::default());
        assert_eq!(
            args,
            vec![
                "install".to_string(),
                "--".to_string(),
                "--switch-ignore-package-manager".to_string(),
            ]
        );
    }

    #[test]
    fn parse_switch_options_rejects_conflicting_flags() {
        let error = parse_switch_options(vec![
            "--switch-ignore-package-manager".to_string(),
            "--switch-version".to_string(),
            "2026.3.12".to_string(),
        ])
        .unwrap_err();

        assert!(
            error
                .to_string()
                .contains("Cannot combine --switch-version with --switch-ignore-package-manager")
        );
    }

    #[test]
    fn select_package_manager_reference_uses_project_reference_by_default() {
        let reference = Some(version_reference("snpm", "2026.3.12"));
        let selected =
            select_package_manager_reference(reference.clone(), &SwitchOptions::default());

        assert_eq!(selected, reference);
    }

    #[test]
    fn select_package_manager_reference_can_ignore_non_snpm_projects() {
        let reference = Some(version_reference("pnpm", "10.0.0"));
        let options = SwitchOptions {
            ignore_package_manager: true,
            version_override: None,
        };

        let selected = select_package_manager_reference(reference, &options);

        assert_eq!(selected, None);
    }

    #[test]
    fn select_package_manager_reference_can_override_project_package_manager() {
        let options = SwitchOptions {
            ignore_package_manager: false,
            version_override: Some("2026.3.12".to_string()),
        };

        let selected =
            select_package_manager_reference(Some(version_reference("pnpm", "10.0.0")), &options);

        assert_eq!(selected, Some(version_reference("snpm", "2026.3.12")));
    }
}

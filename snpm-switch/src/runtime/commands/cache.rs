use crate::cli::SwitchOptions;
use crate::manifest::{self, PackageManagerSpecifier};
use crate::selection::{is_snpm, select_package_manager_reference, warn_non_snpm};
use crate::version;

use std::process::ExitCode;

pub(super) fn run_cache(args: &[String], options: &SwitchOptions) -> anyhow::Result<ExitCode> {
    let requested_versions = parse_cache_versions(args)?;

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

fn parse_cache_versions(args: &[String]) -> anyhow::Result<Vec<&str>> {
    let mut requested_versions = Vec::new();

    for arg in args {
        if arg == "--install" {
            continue;
        }

        if arg.starts_with('-') {
            anyhow::bail!("Unknown switch cache flag: {}", arg);
        }

        requested_versions.push(arg.as_str());
    }

    Ok(requested_versions)
}

fn cache_from_project_or_latest(options: &SwitchOptions) -> anyhow::Result<()> {
    let project_reference = manifest::find_package_manager()?;
    let ignored_project_package_manager =
        options.ignore_package_manager && project_reference.is_some();
    let reference = select_package_manager_reference(project_reference, options);

    match reference {
        Some(reference) if is_snpm(&reference) => match reference.reference {
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
        },
        Some(reference) => {
            warn_non_snpm(&reference);
            println!("Caching latest snpm...");
            version::ensure_latest()?;
            println!("Done.");
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

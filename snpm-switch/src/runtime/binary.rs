use crate::cli::SwitchOptions;
use crate::manifest::{self, PackageManagerReference, PackageManagerSpecifier};
use crate::selection::{is_snpm, select_package_manager_reference, warn_non_snpm};
use crate::version;

use std::path::PathBuf;

pub(super) fn resolve_binary(
    reference: Option<PackageManagerReference>,
    options: &SwitchOptions,
) -> anyhow::Result<PathBuf> {
    let reference = match reference {
        Some(reference) => Some(reference),
        None => manifest::find_package_manager()?,
    };

    match select_package_manager_reference(reference, options) {
        Some(reference) if is_snpm(&reference) => match reference.reference {
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
        },
        Some(reference) => {
            warn_non_snpm(&reference);
            fallback_binary()
        }
        None => fallback_binary(),
    }
}

fn fallback_binary() -> anyhow::Result<PathBuf> {
    if let Some(path) = bundled_binary_path() {
        Ok(path)
    } else {
        version::ensure_latest()
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

use crate::cli::SwitchOptions;
use crate::manifest::{PackageManagerReference, PackageManagerSpecifier};

pub(crate) fn select_package_manager_reference(
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

pub(crate) fn is_snpm(reference: &PackageManagerReference) -> bool {
    reference.name.eq_ignore_ascii_case("snpm")
}

pub(crate) fn warn_non_snpm(reference: &PackageManagerReference) {
    eprintln!(
        "warning: packageManager field specifies '{}', not snpm (use --switch-ignore-package-manager to suppress)",
        reference.name
    );
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

    #[test]
    fn is_snpm_matches_case_insensitively() {
        assert!(is_snpm(&version_reference("snpm", "1.0.0")));
        assert!(is_snpm(&version_reference("SNPM", "1.0.0")));
        assert!(!is_snpm(&version_reference("pnpm", "10.0.0")));
        assert!(!is_snpm(&version_reference("npm", "10.0.0")));
    }
}

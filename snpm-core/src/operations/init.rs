use crate::project::{Manifest, format_manifest};
use crate::{Result, SnpmError, console};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct InitOptions {
    pub package_manager: Option<String>,
}

pub fn init(root: &Path) -> Result<()> {
    init_with_options(root, InitOptions::default())
}

pub fn init_with_options(root: &Path, options: InitOptions) -> Result<()> {
    let manifest_path = root.join("package.json");

    if manifest_path.is_file() {
        return Ok(());
    }

    let manifest = Manifest {
        name: Some(package_name_from_root(root)),
        version: Some("1.0.0".to_string()),
        package_manager: options.package_manager,
        ..Manifest::default()
    };

    let data = format_manifest(&manifest, &manifest_path)?;

    fs::write(&manifest_path, data).map_err(|source| SnpmError::WriteFile {
        path: manifest_path,
        source,
    })?;

    console::info("Created package.json");

    Ok(())
}

fn package_name_from_root(root: &Path) -> String {
    let raw = root.file_name().and_then(|os| os.to_str()).unwrap_or("");
    let normalized = normalize_package_name(raw);

    if normalized.is_empty() {
        "snpm-project".to_string()
    } else {
        normalized
    }
}

fn normalize_package_name(raw: &str) -> String {
    let mut name = String::new();
    let mut last_was_separator = false;

    for ch in raw.trim().chars() {
        let ch = ch.to_ascii_lowercase();

        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            name.push(ch);
            last_was_separator = false;
        } else if !last_was_separator {
            name.push('-');
            last_was_separator = true;
        }
    }

    name.trim_matches(|ch| matches!(ch, '-' | '_' | '.'))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::{InitOptions, init_with_options, normalize_package_name};
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn init_creates_clean_manifest() {
        let temp = tempdir().unwrap();
        let root = temp.path().join("My Great_Project!");
        fs::create_dir(&root).unwrap();

        init_with_options(
            &root,
            InitOptions {
                package_manager: Some("snpm@2026.4.29".to_string()),
            },
        )
        .unwrap();

        let data = fs::read_to_string(root.join("package.json")).unwrap();
        assert_eq!(
            data,
            "{\n  \"name\": \"my-great_project\",\n  \"version\": \"1.0.0\",\n  \"packageManager\": \"snpm@2026.4.29\"\n}\n"
        );
    }

    #[test]
    fn init_does_not_overwrite_existing_manifest() {
        let temp = tempdir().unwrap();
        let manifest_path = temp.path().join("package.json");
        fs::write(&manifest_path, "{\"name\":\"existing\"}\n").unwrap();

        init_with_options(
            temp.path(),
            InitOptions {
                package_manager: Some("snpm@2026.4.29".to_string()),
            },
        )
        .unwrap();

        assert_eq!(
            fs::read_to_string(manifest_path).unwrap(),
            "{\"name\":\"existing\"}\n"
        );
    }

    #[test]
    fn normalize_package_names_for_npm() {
        assert_eq!(normalize_package_name("My App"), "my-app");
        assert_eq!(normalize_package_name("...@#$..."), "");
        assert_eq!(
            normalize_package_name("already.valid_name"),
            "already.valid_name"
        );
    }
}

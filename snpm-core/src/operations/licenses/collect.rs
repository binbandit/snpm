use crate::{Result, SnpmError};

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use super::parse::{LicenseEntry, read_license_from_directory};

pub fn collect_licenses(node_modules: &Path) -> Result<Vec<LicenseEntry>> {
    let virtual_store = node_modules.join(".snpm");

    if !virtual_store.is_dir() {
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let mut seen: BTreeMap<String, bool> = BTreeMap::new();

    let store_entries = fs::read_dir(&virtual_store).map_err(|source| SnpmError::ReadFile {
        path: virtual_store.clone(),
        source,
    })?;

    for entry in store_entries.flatten() {
        let entry_path = entry.path();
        if !entry_path.is_dir() {
            continue;
        }

        let node_modules_dir = entry_path.join("node_modules");
        if !node_modules_dir.is_dir() {
            continue;
        }

        collect_store_packages(&node_modules_dir, &mut seen, &mut entries)?;
    }

    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn collect_store_packages(
    node_modules_dir: &Path,
    seen: &mut BTreeMap<String, bool>,
    entries: &mut Vec<LicenseEntry>,
) -> Result<()> {
    for package_entry in fs::read_dir(node_modules_dir)
        .into_iter()
        .flatten()
        .flatten()
    {
        let package_path = package_entry.path();
        if !package_path.is_dir() {
            continue;
        }

        let dir_name = package_entry.file_name().to_string_lossy().to_string();
        if dir_name.starts_with('@') {
            collect_scoped_packages(&dir_name, &package_path, seen, entries);
        } else {
            push_license_entry(
                read_license_from_directory(&package_path, &dir_name),
                seen,
                entries,
            );
        }
    }

    Ok(())
}

fn collect_scoped_packages(
    scope_name: &str,
    scope_path: &Path,
    seen: &mut BTreeMap<String, bool>,
    entries: &mut Vec<LicenseEntry>,
) {
    for scoped_entry in fs::read_dir(scope_path).into_iter().flatten().flatten() {
        let scoped_path = scoped_entry.path();
        let full_name = format!(
            "{}/{}",
            scope_name,
            scoped_entry.file_name().to_string_lossy()
        );
        push_license_entry(
            read_license_from_directory(&scoped_path, &full_name),
            seen,
            entries,
        );
    }
}

fn push_license_entry(
    license_entry: Option<LicenseEntry>,
    seen: &mut BTreeMap<String, bool>,
    entries: &mut Vec<LicenseEntry>,
) {
    let Some(license_entry) = license_entry else {
        return;
    };

    let key = format!("{}@{}", license_entry.name, license_entry.version);
    if let std::collections::btree_map::Entry::Vacant(vacant) = seen.entry(key) {
        vacant.insert(true);
        entries.push(license_entry);
    }
}

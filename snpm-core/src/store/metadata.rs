use crate::console;
use crate::project::BinField;
use crate::registry::BundledDependencies;
use crate::{Result, SnpmError};

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

pub(crate) const PACKAGE_METADATA_FILE: &str = ".snpm-package-metadata.json";

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageFilesystemShape {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub directories: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub files: Vec<PathBuf>,
}

impl PackageFilesystemShape {
    pub(crate) fn is_empty(&self) -> bool {
        self.directories.is_empty() && self.files.is_empty()
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageStoreMetadata {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_relative_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        rename = "bundledDependencies"
    )]
    pub bundled_dependencies: Option<BundledDependencies>,
    #[serde(default, skip_serializing_if = "is_false", rename = "hasBin")]
    pub has_bin: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<BinField>,
    #[serde(default, skip_serializing_if = "PackageFilesystemShape::is_empty")]
    pub filesystem: PackageFilesystemShape,
}

impl PackageStoreMetadata {
    pub(crate) fn resolve_root(&self, package_dir: &Path) -> Option<PathBuf> {
        let relative = self.root_relative_path.as_deref()?;
        if relative.is_empty() || relative == "." {
            return Some(package_dir.to_path_buf());
        }

        let relative = Path::new(relative);
        if relative.is_absolute() || relative.components().any(invalid_root_component) {
            return None;
        }

        Some(package_dir.join(relative))
    }
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManifestFacts {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    version: Option<String>,
    #[serde(default)]
    bin: Option<BinField>,
    #[serde(default, rename = "bundledDependencies")]
    bundled_dependencies: Option<BundledDependencies>,
    #[serde(default, rename = "bundleDependencies")]
    bundle_dependencies: Option<BundledDependencies>,
}

pub(crate) fn read_package_metadata_lossy(package_root: &Path) -> Option<PackageStoreMetadata> {
    read_metadata_lossy(&package_root.join(PACKAGE_METADATA_FILE))
}

pub(crate) fn read_package_filesystem_shape_lossy(
    package_root: &Path,
) -> Option<PackageFilesystemShape> {
    let metadata = read_package_metadata_lossy(package_root)?;
    (!metadata.filesystem.is_empty()).then_some(metadata.filesystem)
}

pub(in crate::store) fn read_store_package_metadata_lossy(
    package_dir: &Path,
) -> Option<PackageStoreMetadata> {
    read_metadata_lossy(&package_dir.join(PACKAGE_METADATA_FILE))
}

pub(in crate::store) fn persist_package_metadata(
    package_dir: &Path,
    package_root: &Path,
) -> Result<()> {
    let tree_index = collect_package_tree_index(package_root)?;
    let mut root_metadata = collect_manifest_facts(package_root);
    root_metadata.filesystem = tree_index.filesystem.clone();
    write_metadata(&package_root.join(PACKAGE_METADATA_FILE), &root_metadata)?;

    for nested_root in &tree_index.nested_package_roots {
        let nested_package_root = package_root.join(nested_root);
        let nested_metadata = collect_manifest_facts(&nested_package_root);
        write_metadata(
            &nested_package_root.join(PACKAGE_METADATA_FILE),
            &nested_metadata,
        )?;
    }

    let Some(relative_root) = root_relative_path(package_dir, package_root) else {
        return Ok(());
    };

    if package_dir == package_root {
        return Ok(());
    }

    root_metadata.root_relative_path = Some(relative_root);
    root_metadata.filesystem = PackageFilesystemShape::default();
    write_metadata(&package_dir.join(PACKAGE_METADATA_FILE), &root_metadata)
}

#[derive(Default)]
struct PackageTreeIndex {
    filesystem: PackageFilesystemShape,
    nested_package_roots: BTreeSet<PathBuf>,
}

fn collect_package_tree_index(package_root: &Path) -> Result<PackageTreeIndex> {
    let mut index = PackageTreeIndex::default();
    collect_directory_shape(package_root, package_root, Path::new(""), &mut index)?;

    index
        .filesystem
        .files
        .push(PathBuf::from(PACKAGE_METADATA_FILE));
    for nested_root in &index.nested_package_roots {
        index
            .filesystem
            .files
            .push(nested_root.join(PACKAGE_METADATA_FILE));
    }

    index.filesystem.directories.sort();
    index.filesystem.directories.dedup();
    index.filesystem.files.sort();
    index.filesystem.files.dedup();

    Ok(index)
}

fn collect_directory_shape(
    package_root: &Path,
    current: &Path,
    relative: &Path,
    index: &mut PackageTreeIndex,
) -> Result<()> {
    let mut entries: Vec<_> = fs::read_dir(current)
        .map_err(|source| SnpmError::ReadFile {
            path: current.to_path_buf(),
            source,
        })?
        .collect::<std::result::Result<_, _>>()
        .map_err(|source| SnpmError::ReadFile {
            path: current.to_path_buf(),
            source,
        })?;

    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let name = entry.file_name();
        if name == PACKAGE_METADATA_FILE {
            continue;
        }

        let entry_type = entry.file_type().map_err(|source| SnpmError::ReadFile {
            path: entry.path(),
            source,
        })?;
        let entry_path = entry.path();
        let entry_relative = join_relative(relative, &name);

        if entry_type.is_symlink() {
            return Err(SnpmError::Io {
                path: entry_path,
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "refusing to index symlink from package store",
                ),
            });
        }

        if entry_type.is_dir() {
            index.filesystem.directories.push(entry_relative.clone());
            collect_directory_shape(package_root, &entry_path, &entry_relative, index)?;
            continue;
        }

        index.filesystem.files.push(entry_relative.clone());
        if name == "package.json"
            && let Some(parent) = entry_path.parent()
            && parent != package_root
        {
            index.nested_package_roots.insert(
                entry_relative
                    .parent()
                    .unwrap_or(Path::new(""))
                    .to_path_buf(),
            );
        }
    }

    Ok(())
}

fn join_relative(parent: &Path, child: &std::ffi::OsStr) -> PathBuf {
    if parent.as_os_str().is_empty() {
        PathBuf::from(child)
    } else {
        parent.join(child)
    }
}

fn collect_manifest_facts(package_root: &Path) -> PackageStoreMetadata {
    let manifest_path = package_root.join("package.json");
    if !manifest_path.is_file() {
        return PackageStoreMetadata::default();
    }

    let data = match fs::read_to_string(&manifest_path) {
        Ok(data) => data,
        Err(error) => {
            console::verbose(&format!(
                "failed to read package metadata manifest {}: {}",
                manifest_path.display(),
                error
            ));
            return PackageStoreMetadata::default();
        }
    };

    let manifest = match serde_json::from_str::<ManifestFacts>(&data) {
        Ok(manifest) => manifest,
        Err(error) => {
            console::verbose(&format!(
                "failed to parse package metadata manifest {}: {}",
                manifest_path.display(),
                error
            ));
            return PackageStoreMetadata::default();
        }
    };

    PackageStoreMetadata {
        root_relative_path: None,
        name: manifest.name,
        version: manifest.version,
        bundled_dependencies: manifest
            .bundled_dependencies
            .or(manifest.bundle_dependencies),
        has_bin: manifest.bin.is_some(),
        bin: manifest.bin,
        filesystem: PackageFilesystemShape::default(),
    }
}

fn root_relative_path(package_dir: &Path, package_root: &Path) -> Option<String> {
    let relative = package_root.strip_prefix(package_dir).ok()?;
    if relative.as_os_str().is_empty() {
        return None;
    }

    Some(relative.to_string_lossy().to_string())
}

fn read_metadata_lossy(path: &Path) -> Option<PackageStoreMetadata> {
    let data = fs::read_to_string(path).ok()?;
    serde_json::from_str(&data).ok()
}

fn write_metadata(path: &Path, metadata: &PackageStoreMetadata) -> Result<()> {
    let data = serde_json::to_vec(metadata).map_err(|source| SnpmError::SerializeJson {
        path: path.to_path_buf(),
        reason: source.to_string(),
    })?;
    fs::write(path, data).map_err(|source| SnpmError::WriteFile {
        path: path.to_path_buf(),
        source,
    })
}

fn invalid_root_component(component: Component<'_>) -> bool {
    matches!(
        component,
        Component::ParentDir | Component::RootDir | Component::Prefix(_)
    )
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[cfg(test)]
mod tests {
    use super::{
        PACKAGE_METADATA_FILE, persist_package_metadata, read_package_filesystem_shape_lossy,
        read_package_metadata_lossy, read_store_package_metadata_lossy,
    };
    use crate::project::BinField;
    use crate::registry::BundledDependencies;

    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use tempfile::tempdir;

    #[test]
    fn persist_package_metadata_writes_root_and_store_metadata() {
        let temp = tempdir().unwrap();
        let package_dir = temp.path();
        let package_root = package_dir.join("package");
        fs::create_dir_all(&package_root).unwrap();
        fs::write(
            package_root.join("package.json"),
            r#"{
                "name": "tool",
                "version": "1.2.3",
                "bin": {
                    "tool": "cli.js"
                },
                "bundledDependencies": ["dep-a"]
            }"#,
        )
        .unwrap();

        persist_package_metadata(package_dir, &package_root).unwrap();

        let root_metadata = read_package_metadata_lossy(&package_root).unwrap();
        assert_eq!(root_metadata.name.as_deref(), Some("tool"));
        assert_eq!(root_metadata.version.as_deref(), Some("1.2.3"));
        assert_eq!(
            root_metadata.bin,
            Some(BinField::Map(BTreeMap::from([(
                "tool".to_string(),
                "cli.js".to_string(),
            )])))
        );
        assert_eq!(
            root_metadata.bundled_dependencies,
            Some(BundledDependencies::List(vec!["dep-a".to_string()]))
        );
        assert!(root_metadata.root_relative_path.is_none());
        assert!(
            root_metadata
                .filesystem
                .files
                .contains(&PathBuf::from("package.json"))
        );
        assert!(
            root_metadata
                .filesystem
                .files
                .contains(&PathBuf::from(PACKAGE_METADATA_FILE))
        );

        let store_metadata = read_store_package_metadata_lossy(package_dir).unwrap();
        assert_eq!(
            store_metadata.root_relative_path.as_deref(),
            Some("package")
        );
        assert!(store_metadata.filesystem.is_empty());
    }

    #[test]
    fn persist_package_metadata_writes_single_file_for_flat_package() {
        let temp = tempdir().unwrap();
        let package_dir = temp.path();
        fs::write(
            package_dir.join("package.json"),
            r#"{ "name": "flat", "version": "1.0.0" }"#,
        )
        .unwrap();

        persist_package_metadata(package_dir, package_dir).unwrap();

        assert!(package_dir.join(PACKAGE_METADATA_FILE).is_file());
        let metadata = read_store_package_metadata_lossy(package_dir).unwrap();
        assert_eq!(metadata.name.as_deref(), Some("flat"));
        assert!(metadata.root_relative_path.is_none());
        assert!(
            metadata
                .filesystem
                .files
                .contains(&PathBuf::from("package.json"))
        );
    }

    #[test]
    fn persist_package_metadata_indexes_filesystem_shape_and_nested_packages() {
        let temp = tempdir().unwrap();
        let package_root = temp.path().join("package");
        let bundled_root = package_root.join("node_modules/dep");
        fs::create_dir_all(package_root.join("lib")).unwrap();
        fs::create_dir_all(&bundled_root).unwrap();

        fs::write(
            package_root.join("package.json"),
            r#"{ "name": "root", "version": "1.0.0" }"#,
        )
        .unwrap();
        fs::write(package_root.join("lib/index.js"), "export {};\n").unwrap();
        fs::write(
            bundled_root.join("package.json"),
            r#"{ "name": "dep", "version": "1.0.0", "bin": "cli.js" }"#,
        )
        .unwrap();
        fs::write(bundled_root.join("cli.js"), "#!/usr/bin/env node\n").unwrap();

        persist_package_metadata(temp.path(), &package_root).unwrap();

        let shape = read_package_filesystem_shape_lossy(&package_root).unwrap();
        assert!(shape.directories.contains(&PathBuf::from("lib")));
        assert!(shape.directories.contains(&PathBuf::from("node_modules")));
        assert!(
            shape
                .directories
                .contains(&PathBuf::from("node_modules/dep"))
        );
        assert!(shape.files.contains(&PathBuf::from("lib/index.js")));
        assert!(shape.files.contains(&PathBuf::from(PACKAGE_METADATA_FILE)));
        assert!(shape.files.contains(&PathBuf::from(
            "node_modules/dep/.snpm-package-metadata.json"
        )));

        let nested_metadata = read_package_metadata_lossy(&bundled_root).unwrap();
        assert_eq!(nested_metadata.name.as_deref(), Some("dep"));
        assert_eq!(
            nested_metadata.bin,
            Some(BinField::Single("cli.js".to_string()))
        );
    }
}

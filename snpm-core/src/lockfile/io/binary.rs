//! Binary sidecar for `snpm-lock.yaml`.
//!
//! The YAML lockfile remains canonical: it's the diffable, human-editable
//! source of truth. Whenever we write the YAML we also write a binary file
//! beside it (`snpm-lock.bin`) that contains a bincode-encoded copy of the
//! same `Lockfile` struct, tagged with a SHA-256 of the YAML bytes the
//! sidecar was derived from.
//!
//! On read we try the binary first; if its embedded hash matches a SHA-256 of
//! the YAML on disk, we deserialize from binary and skip YAML parsing
//! entirely. Any mismatch (missing sidecar, corrupt header, stale hash after
//! a manual edit, schema drift) falls back to YAML.
//!
//! The binary body uses mirror types rather than the `Lockfile` structs
//! directly: bincode is positional, so `skip_serializing_if` fields would
//! misalign the stream, and the `#[serde(untagged)]` enums (`BinField`,
//! `BundledDependencies`) can't be decoded from a non-self-describing
//! format at all. The mirrors serialize every field and use ordinary
//! tagged enums.

use super::super::types::{LockPackage, LockRoot, LockRootDependency, Lockfile};
use crate::project::BinField;
use crate::registry::BundledDependencies;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const MAGIC: [u8; 4] = *b"SNPB";
const FORMAT_VERSION: u32 = 3;
const HEADER_LEN: usize = 4 + 4 + 32;

#[derive(Serialize, Deserialize)]
struct BinLockfile {
    version: u32,
    root: BTreeMap<String, BinRootDependency>,
    packages: BTreeMap<String, BinPackage>,
}

#[derive(Serialize, Deserialize)]
struct BinRootDependency {
    requested: String,
    package: Option<String>,
    version: Option<String>,
    optional: bool,
}

#[derive(Serialize, Deserialize)]
struct BinPackage {
    name: String,
    version: String,
    tarball: String,
    integrity: Option<String>,
    dependencies: BTreeMap<String, String>,
    peer_dependencies: BTreeMap<String, String>,
    bundled_dependencies: Option<BinBundledDependencies>,
    has_bin: bool,
    bin: Option<BinBinField>,
}

#[derive(Serialize, Deserialize)]
enum BinBundledDependencies {
    List(Vec<String>),
    All(bool),
}

#[derive(Serialize, Deserialize)]
enum BinBinField {
    Single(String),
    Map(BTreeMap<String, String>),
}

impl From<&Lockfile> for BinLockfile {
    fn from(lockfile: &Lockfile) -> Self {
        Self {
            version: lockfile.version,
            root: lockfile
                .root
                .dependencies
                .iter()
                .map(|(name, dep)| {
                    (
                        name.clone(),
                        BinRootDependency {
                            requested: dep.requested.clone(),
                            package: dep.package.clone(),
                            version: dep.version.clone(),
                            optional: dep.optional,
                        },
                    )
                })
                .collect(),
            packages: lockfile
                .packages
                .iter()
                .map(|(key, package)| {
                    (
                        key.clone(),
                        BinPackage {
                            name: package.name.clone(),
                            version: package.version.clone(),
                            tarball: package.tarball.clone(),
                            integrity: package.integrity.clone(),
                            dependencies: package.dependencies.clone(),
                            peer_dependencies: package.peer_dependencies.clone(),
                            bundled_dependencies: package.bundled_dependencies.as_ref().map(
                                |bundled| match bundled {
                                    BundledDependencies::List(list) => {
                                        BinBundledDependencies::List(list.clone())
                                    }
                                    BundledDependencies::All(all) => {
                                        BinBundledDependencies::All(*all)
                                    }
                                },
                            ),
                            has_bin: package.has_bin,
                            bin: package.bin.as_ref().map(|bin| match bin {
                                BinField::Single(value) => BinBinField::Single(value.clone()),
                                BinField::Map(map) => BinBinField::Map(map.clone()),
                            }),
                        },
                    )
                })
                .collect(),
        }
    }
}

impl From<BinLockfile> for Lockfile {
    fn from(binary: BinLockfile) -> Self {
        Self {
            version: binary.version,
            root: LockRoot {
                dependencies: binary
                    .root
                    .into_iter()
                    .map(|(name, dep)| {
                        (
                            name,
                            LockRootDependency {
                                requested: dep.requested,
                                package: dep.package,
                                version: dep.version,
                                optional: dep.optional,
                            },
                        )
                    })
                    .collect(),
            },
            packages: binary
                .packages
                .into_iter()
                .map(|(key, package)| {
                    (
                        key,
                        LockPackage {
                            name: package.name,
                            version: package.version,
                            tarball: package.tarball,
                            integrity: package.integrity,
                            dependencies: package.dependencies,
                            peer_dependencies: package.peer_dependencies,
                            bundled_dependencies: package.bundled_dependencies.map(|bundled| {
                                match bundled {
                                    BinBundledDependencies::List(list) => {
                                        BundledDependencies::List(list)
                                    }
                                    BinBundledDependencies::All(all) => {
                                        BundledDependencies::All(all)
                                    }
                                }
                            }),
                            has_bin: package.has_bin,
                            bin: package.bin.map(|bin| match bin {
                                BinBinField::Single(value) => BinField::Single(value),
                                BinBinField::Map(map) => BinField::Map(map),
                            }),
                        },
                    )
                })
                .collect(),
        }
    }
}

pub(super) fn sidecar_path(yaml_path: &Path) -> PathBuf {
    yaml_path.with_extension("bin")
}

pub(super) fn yaml_hash(yaml_bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(yaml_bytes);
    let digest = hasher.finalize();
    let mut out = [0_u8; 32];
    out.copy_from_slice(&digest);
    out
}

pub(super) fn encode_sidecar(lockfile: &Lockfile, yaml_hash: [u8; 32]) -> Option<Vec<u8>> {
    let body = bincode::serialize(&BinLockfile::from(lockfile)).ok()?;
    let mut out = Vec::with_capacity(HEADER_LEN + body.len());
    out.extend_from_slice(&MAGIC);
    out.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    out.extend_from_slice(&yaml_hash);
    out.extend_from_slice(&body);
    Some(out)
}

/// Try to deserialize a sidecar that's expected to describe `expected_yaml_hash`.
/// Returns `None` for any mismatch — the caller should fall back to YAML
/// parsing rather than surfacing an error.
pub(super) fn decode_sidecar(bytes: &[u8], expected_yaml_hash: [u8; 32]) -> Option<Lockfile> {
    if bytes.len() < HEADER_LEN {
        return None;
    }
    if bytes[..4] != MAGIC {
        return None;
    }
    let version = u32::from_le_bytes(bytes[4..8].try_into().ok()?);
    if version != FORMAT_VERSION {
        return None;
    }
    let stored_hash: [u8; 32] = bytes[8..40].try_into().ok()?;
    if stored_hash != expected_yaml_hash {
        return None;
    }
    bincode::deserialize::<BinLockfile>(&bytes[HEADER_LEN..])
        .ok()
        .map(Lockfile::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lockfile::types::{LockPackage, LockRoot, LockRootDependency, Lockfile};
    use crate::project::BinField;
    use crate::registry::BundledDependencies;
    use std::collections::BTreeMap;

    fn sample_lockfile() -> Lockfile {
        Lockfile {
            version: 1,
            root: LockRoot {
                dependencies: BTreeMap::new(),
            },
            packages: BTreeMap::new(),
        }
    }

    /// A lockfile shaped like real ones: root deps with skipped
    /// (`None`/`false`) fields, packages with and without bins,
    /// untagged-enum values in both variants.
    fn realistic_lockfile() -> Lockfile {
        let mut root_deps = BTreeMap::new();
        root_deps.insert(
            "is-odd".to_string(),
            LockRootDependency {
                requested: "^3.0.0".to_string(),
                package: None,
                version: Some("3.0.1".to_string()),
                optional: false,
            },
        );
        root_deps.insert(
            "aliased".to_string(),
            LockRootDependency {
                requested: "npm:real@^1.0.0".to_string(),
                package: Some("real".to_string()),
                version: Some("1.0.0".to_string()),
                optional: true,
            },
        );

        let mut packages = BTreeMap::new();
        packages.insert(
            "is-odd@3.0.1".to_string(),
            LockPackage {
                name: "is-odd".to_string(),
                version: "3.0.1".to_string(),
                tarball: "https://registry.npmjs.org/is-odd/-/is-odd-3.0.1.tgz".to_string(),
                integrity: Some("sha512-abc".to_string()),
                dependencies: BTreeMap::from([(
                    "is-number".to_string(),
                    "is-number@6.0.0".to_string(),
                )]),
                peer_dependencies: BTreeMap::new(),
                bundled_dependencies: None,
                has_bin: false,
                bin: None,
            },
        );
        packages.insert(
            "tool@1.0.0".to_string(),
            LockPackage {
                name: "tool".to_string(),
                version: "1.0.0".to_string(),
                tarball: "https://registry.npmjs.org/tool/-/tool-1.0.0.tgz".to_string(),
                integrity: None,
                dependencies: BTreeMap::new(),
                peer_dependencies: BTreeMap::new(),
                bundled_dependencies: Some(BundledDependencies::List(vec!["vendored".to_string()])),
                has_bin: true,
                bin: Some(BinField::Single("cli.js".to_string())),
            },
        );
        packages.insert(
            "multi@2.0.0".to_string(),
            LockPackage {
                name: "multi".to_string(),
                version: "2.0.0".to_string(),
                tarball: "https://registry.npmjs.org/multi/-/multi-2.0.0.tgz".to_string(),
                integrity: Some("sha512-def".to_string()),
                dependencies: BTreeMap::new(),
                peer_dependencies: BTreeMap::new(),
                bundled_dependencies: Some(BundledDependencies::All(true)),
                has_bin: true,
                bin: Some(BinField::Map(BTreeMap::from([(
                    "multi".to_string(),
                    "bin/multi.js".to_string(),
                )]))),
            },
        );

        Lockfile {
            version: 1,
            root: LockRoot {
                dependencies: root_deps,
            },
            packages,
        }
    }

    #[test]
    fn round_trip_matches() {
        let lockfile = sample_lockfile();
        let yaml = b"version: 1\nroot:\n  dependencies: {}\npackages: {}\n";
        let hash = yaml_hash(yaml);
        let bytes = encode_sidecar(&lockfile, hash).unwrap();
        let decoded = decode_sidecar(&bytes, hash).unwrap();
        assert_eq!(decoded, lockfile);
    }

    #[test]
    fn round_trip_matches_with_skipped_fields_and_untagged_enums() {
        // Regression: the old sidecar serialized `Lockfile` directly.
        // `skip_serializing_if` fields misaligned the positional bincode
        // stream and the untagged enums couldn't decode at all, so every
        // real-world sidecar failed to load and silently fell back to
        // YAML.
        let lockfile = realistic_lockfile();
        let hash = yaml_hash(b"whatever");
        let bytes = encode_sidecar(&lockfile, hash).unwrap();
        let decoded = decode_sidecar(&bytes, hash).unwrap();
        assert_eq!(decoded, lockfile);
    }

    #[test]
    fn decode_returns_none_when_yaml_hash_drifts() {
        let lockfile = sample_lockfile();
        let bytes = encode_sidecar(&lockfile, yaml_hash(b"original")).unwrap();
        assert!(decode_sidecar(&bytes, yaml_hash(b"edited")).is_none());
    }

    #[test]
    fn decode_returns_none_for_bad_magic() {
        let lockfile = sample_lockfile();
        let mut bytes = encode_sidecar(&lockfile, yaml_hash(b"x")).unwrap();
        bytes[0] = b'!';
        assert!(decode_sidecar(&bytes, yaml_hash(b"x")).is_none());
    }

    #[test]
    fn decode_returns_none_for_truncated_header() {
        assert!(decode_sidecar(&[0_u8; 8], [0_u8; 32]).is_none());
    }
}

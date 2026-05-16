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

use super::super::types::Lockfile;

use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

const MAGIC: [u8; 4] = *b"SNPB";
const FORMAT_VERSION: u32 = 1;
const HEADER_LEN: usize = 4 + 4 + 32;

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
    let body = bincode::serialize(lockfile).ok()?;
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
    bincode::deserialize::<Lockfile>(&bytes[HEADER_LEN..]).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lockfile::types::{LockRoot, Lockfile};
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

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::sync::{Mutex, OnceLock};

/// Tri-state knowledge about whether a link primitive works for a
/// specific `(source_fs, dest_fs)` pair within this process.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Probe {
    #[default]
    Untested,
    Capable,
    Incapable,
}

#[derive(Debug, Clone, Copy, Default)]
struct Capabilities {
    reflink: Probe,
    hardlink: Probe,
}

/// Per-`(source_dev_id, dest_dev_id)` memo so we don't pay the syscall
/// cost of re-attempting a primitive that already failed on this
/// filesystem pair. We key by device-id pair (not "global per process")
/// so reflink across filesystems doesn't poison reflink within them —
/// a project that places its `snpm` cache on APFS and its `node_modules`
/// on an external ext4 mount can still reflink within the APFS half.
static CACHE: OnceLock<Mutex<HashMap<(u64, u64), Capabilities>>> = OnceLock::new();

fn cache() -> &'static Mutex<HashMap<(u64, u64), Capabilities>> {
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn key_for(from: &Path, to: &Path) -> io::Result<(u64, u64)> {
    Ok((device_id(from)?, device_id_for_create(to)?))
}

#[cfg(unix)]
fn device_id(path: &Path) -> io::Result<u64> {
    use std::os::unix::fs::MetadataExt;
    std::fs::metadata(path).map(|metadata| metadata.dev())
}

#[cfg(unix)]
fn device_id_for_create(path: &Path) -> io::Result<u64> {
    // The destination may not exist yet; stat the deepest existing
    // ancestor to learn which filesystem the new file would live on.
    use std::os::unix::fs::MetadataExt;
    let mut candidate = path;
    loop {
        match std::fs::metadata(candidate) {
            Ok(metadata) => return Ok(metadata.dev()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                candidate = candidate.parent().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::NotFound, "no ancestor found for path")
                })?;
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(not(unix))]
fn device_id(_path: &Path) -> io::Result<u64> {
    Ok(0)
}

#[cfg(not(unix))]
fn device_id_for_create(_path: &Path) -> io::Result<u64> {
    Ok(0)
}

/// Returns `true` if a previous reflink attempt for this `(src_fs, dst_fs)`
/// pair succeeded, or no attempt has been made yet. Returns `false` only
/// after a recorded failure — callers should then skip the syscall and
/// go straight to the fallback path.
pub(crate) fn reflink_likely(from: &Path, to: &Path) -> bool {
    match key_for(from, to) {
        Ok(key) => {
            let cache = cache().lock().unwrap();
            cache
                .get(&key)
                .map(|caps| caps.reflink != Probe::Incapable)
                .unwrap_or(true)
        }
        // If we can't even stat the source, let the caller's attempt
        // surface the real error instead of silently downgrading.
        Err(_) => true,
    }
}

pub(crate) fn record_reflink_outcome(from: &Path, to: &Path, succeeded: bool) {
    let Ok(key) = key_for(from, to) else {
        return;
    };
    let mut cache = cache().lock().unwrap();
    let caps = cache.entry(key).or_default();
    caps.reflink = if succeeded {
        Probe::Capable
    } else {
        Probe::Incapable
    };
}

pub(crate) fn hardlink_likely(from: &Path, to: &Path) -> bool {
    match key_for(from, to) {
        Ok(key) => {
            let cache = cache().lock().unwrap();
            cache
                .get(&key)
                .map(|caps| caps.hardlink != Probe::Incapable)
                .unwrap_or(true)
        }
        Err(_) => true,
    }
}

pub(crate) fn record_hardlink_outcome(from: &Path, to: &Path, succeeded: bool) {
    let Ok(key) = key_for(from, to) else {
        return;
    };
    let mut cache = cache().lock().unwrap();
    let caps = cache.entry(key).or_default();
    caps.hardlink = if succeeded {
        Probe::Capable
    } else {
        Probe::Incapable
    };
}

#[cfg(test)]
pub(crate) fn reset_for_tests() {
    if let Some(lock) = CACHE.get() {
        lock.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn reflink_likely_defaults_to_true() {
        reset_for_tests();
        let dir = tempdir().unwrap();
        let from = dir.path().join("from.txt");
        let to = dir.path().join("to.txt");
        fs::write(&from, b"x").unwrap();

        assert!(reflink_likely(&from, &to));
    }

    #[test]
    fn record_failure_blocks_subsequent_attempts() {
        reset_for_tests();
        let dir = tempdir().unwrap();
        let from = dir.path().join("from.txt");
        let to = dir.path().join("to.txt");
        fs::write(&from, b"x").unwrap();

        record_reflink_outcome(&from, &to, false);
        assert!(!reflink_likely(&from, &to));

        // Different path on the same fs is still keyed by (dev, dev) so
        // it's blocked too — that's the point.
        let other_to = dir.path().join("other.txt");
        assert!(!reflink_likely(&from, &other_to));
    }

    #[test]
    fn record_success_overrides_prior_failure() {
        reset_for_tests();
        let dir = tempdir().unwrap();
        let from = dir.path().join("from.txt");
        let to = dir.path().join("to.txt");
        fs::write(&from, b"x").unwrap();

        record_reflink_outcome(&from, &to, false);
        record_reflink_outcome(&from, &to, true);
        assert!(reflink_likely(&from, &to));
    }

    #[test]
    fn hardlink_state_is_independent_of_reflink_state() {
        reset_for_tests();
        let dir = tempdir().unwrap();
        let from = dir.path().join("from.txt");
        let to = dir.path().join("to.txt");
        fs::write(&from, b"x").unwrap();

        record_reflink_outcome(&from, &to, false);
        assert!(hardlink_likely(&from, &to));
        record_hardlink_outcome(&from, &to, false);
        assert!(!hardlink_likely(&from, &to));
    }
}

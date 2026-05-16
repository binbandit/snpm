use super::super::types::{LOCKFILE_VERSION, Lockfile};
use super::binary::{decode_sidecar, sidecar_path, yaml_hash};
use crate::{Result, SnpmError};

use std::fs;
use std::path::Path;

pub fn read(path: &Path) -> Result<Lockfile> {
    let data = fs::read(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    // Try the binary sidecar first. Its embedded hash is checked against the
    // current YAML, so a manual edit (or a snpm version that wrote the YAML
    // without a sidecar) cleanly falls back to YAML parsing.
    let sidecar = sidecar_path(path);
    if let Ok(bin_bytes) = fs::read(&sidecar)
        && let Some(lockfile) = decode_sidecar(&bin_bytes, yaml_hash(&data))
    {
        check_version(&lockfile, path)?;
        return Ok(lockfile);
    }

    let yaml = std::str::from_utf8(&data).map_err(|error| SnpmError::Lockfile {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    let lockfile: Lockfile = serde_yaml::from_str(yaml).map_err(|error| SnpmError::Lockfile {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;
    check_version(&lockfile, path)?;
    Ok(lockfile)
}

fn check_version(lockfile: &Lockfile, path: &Path) -> Result<()> {
    if lockfile.version != LOCKFILE_VERSION {
        return Err(SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: format!(
                "unsupported lockfile version {} (expected {}), delete the lockfile and reinstall",
                lockfile.version, LOCKFILE_VERSION
            ),
        });
    }
    Ok(())
}

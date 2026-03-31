use super::super::types::{LOCKFILE_VERSION, Lockfile};
use crate::{Result, SnpmError};

use std::fs;
use std::path::Path;

pub fn read(path: &Path) -> Result<Lockfile> {
    let data = fs::read_to_string(path).map_err(|source| SnpmError::ReadFile {
        path: path.to_path_buf(),
        source,
    })?;

    let lockfile: Lockfile = serde_yaml::from_str(&data).map_err(|error| SnpmError::Lockfile {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;

    if lockfile.version != LOCKFILE_VERSION {
        return Err(SnpmError::Lockfile {
            path: path.to_path_buf(),
            reason: format!(
                "unsupported lockfile version {} (expected {}), delete the lockfile and reinstall",
                lockfile.version, LOCKFILE_VERSION
            ),
        });
    }

    Ok(lockfile)
}

use crate::{Result, SnpmError};
use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tar::Builder;

pub(super) fn write_tarball(
    output_dir: &Path,
    tarball_path: &Path,
    project_root: &Path,
    files: &[PathBuf],
) -> Result<u64> {
    let tar_data = Vec::new();
    let mut builder = Builder::new(tar_data);

    for file_path in files {
        append_file(&mut builder, file_path, project_root, tarball_path)?;
    }

    let tar_bytes = builder.into_inner().map_err(|error| SnpmError::Archive {
        path: tarball_path.to_path_buf(),
        source: std::io::Error::other(error.to_string()),
    })?;

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&tar_bytes)
        .map_err(|source| SnpmError::WriteFile {
            path: tarball_path.to_path_buf(),
            source,
        })?;

    let compressed = encoder.finish().map_err(|source| SnpmError::WriteFile {
        path: tarball_path.to_path_buf(),
        source,
    })?;

    fs::create_dir_all(output_dir).map_err(|source| SnpmError::WriteFile {
        path: output_dir.to_path_buf(),
        source,
    })?;

    fs::write(tarball_path, &compressed).map_err(|source| SnpmError::WriteFile {
        path: tarball_path.to_path_buf(),
        source,
    })?;

    Ok(compressed.len() as u64)
}

fn append_file(
    builder: &mut Builder<Vec<u8>>,
    file_path: &Path,
    project_root: &Path,
    tarball_path: &Path,
) -> Result<()> {
    let rel_path = file_path.strip_prefix(project_root).unwrap_or(file_path);
    let archive_path = Path::new("package").join(rel_path);
    let metadata = fs::metadata(file_path).map_err(|source| SnpmError::ReadFile {
        path: file_path.to_path_buf(),
        source,
    })?;

    if !metadata.is_file() {
        return Ok(());
    }

    let data = fs::read(file_path).map_err(|source| SnpmError::ReadFile {
        path: file_path.to_path_buf(),
        source,
    })?;

    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o644);
    header.set_mtime(0);
    header.set_cksum();

    builder
        .append_data(&mut header, &archive_path, data.as_slice())
        .map_err(|source| SnpmError::Archive {
            path: tarball_path.to_path_buf(),
            source,
        })?;

    Ok(())
}

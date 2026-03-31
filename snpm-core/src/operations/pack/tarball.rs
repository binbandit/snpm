use crate::{Result, SnpmError};
use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
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
    header.set_mode(archive_mode(&metadata));
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

#[cfg(unix)]
fn archive_mode(metadata: &fs::Metadata) -> u32 {
    metadata.permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn archive_mode(_metadata: &fs::Metadata) -> u32 {
    0o644
}

#[cfg(test)]
mod tests {
    use super::write_tarball;
    use flate2::read::GzDecoder;
    use std::fs;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tar::Archive;
    use tempfile::tempdir;

    #[cfg(unix)]
    #[test]
    fn write_tarball_preserves_executable_mode() {
        let dir = tempdir().unwrap();
        let project_root = dir.path().join("project");
        let output_dir = dir.path().join("out");
        let tarball_path = output_dir.join("pkg.tgz");
        let bin_path = project_root.join("bin").join("cli.js");

        fs::create_dir_all(bin_path.parent().unwrap()).unwrap();
        fs::write(&bin_path, "#!/usr/bin/env node\n").unwrap();

        let mut permissions = fs::metadata(&bin_path).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&bin_path, permissions).unwrap();

        write_tarball(
            &output_dir,
            &tarball_path,
            &project_root,
            std::slice::from_ref(&bin_path),
        )
        .unwrap();

        let archive_data = fs::read(&tarball_path).unwrap();
        let decoder = GzDecoder::new(archive_data.as_slice());
        let mut archive = Archive::new(decoder);
        let mut entries = archive.entries().unwrap();
        let entry = entries.next().unwrap().unwrap();

        assert_eq!(entry.header().mode().unwrap() & 0o777, 0o755);
    }
}

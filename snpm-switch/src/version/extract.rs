use flate2::read::GzDecoder;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use tar::Archive;

pub(super) fn extract_tarball(data: &[u8], destination: &Path) -> anyhow::Result<()> {
    let cursor = Cursor::new(data);
    let decoder = GzDecoder::new(cursor);
    let mut archive = Archive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();

        if file_name == "snpm" || file_name == "snpm.exe" {
            let dest_path = destination.join(file_name);
            entry.unpack(&dest_path)?;
            set_executable_if_needed(&dest_path)?;
            return Ok(());
        }
    }

    anyhow::bail!("snpm binary not found in archive");
}

pub(super) fn extract_zip(data: &[u8], destination: &Path) -> anyhow::Result<()> {
    let cursor = Cursor::new(data);
    let mut archive = zip::ZipArchive::new(cursor)?;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index)?;
        let file_name = Path::new(file.name())
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();

        if file_name == "snpm" || file_name == "snpm.exe" {
            let dest_path = destination.join(file_name);
            let mut dest_file = fs::File::create(&dest_path)?;
            std::io::copy(&mut file, &mut dest_file)?;
            set_executable_if_needed(&dest_path)?;
            return Ok(());
        }
    }

    anyhow::bail!("snpm binary not found in archive");
}

fn set_executable_if_needed(path: &Path) -> anyhow::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut permissions = fs::metadata(path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions)?;
    }

    Ok(())
}

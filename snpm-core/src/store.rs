use crate::resolve::ResolvedPackage;
use crate::{Result, SnpmConfig, SnpmError};
use flate2::read::GzDecoder;
use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use tar::Archive;

pub async fn ensure_package(
    config: &SnpmConfig,
    package: &ResolvedPackage,
    client: &reqwest::Client,
) -> Result<PathBuf> {
    let base = config.packages_dir();
    let name_dir = sanitize_name(&package.id.name);
    let pkg_dir = base.join(name_dir).join(&package.id.version);

    let marker = pkg_dir.join(".snpm_complete");
    if marker.is_file() {
        return Ok(package_root_dir(&pkg_dir));
    }

    fs::create_dir_all(&pkg_dir).map_err(|source| SnpmError::WriteFile {
        path: pkg_dir.clone(),
        source,
    })?;

    let bytes = download_tarball(&package.tarball, client).await?;
    unpack_tarball(&pkg_dir, bytes)?;

    fs::write(&marker, []).map_err(|source| SnpmError::WriteFile {
        path: marker.clone(),
        source,
    })?;

    Ok(package_root_dir(&pkg_dir))
}

fn sanitize_name(name: &str) -> String {
    name.replace('/', "_")
}

fn package_root_dir(pkg_dir: &PathBuf) -> PathBuf {
    let candidate = pkg_dir.join("package");
    if candidate.is_dir() {
        candidate
    } else {
        pkg_dir.clone()
    }
}

async fn download_tarball(url: &str, client: &reqwest::Client) -> Result<Vec<u8>> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?;

    let bytes = response
        .error_for_status()
        .map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?
        .bytes()
        .await
        .map_err(|source| SnpmError::Http {
            url: url.to_string(),
            source,
        })?;

    Ok(bytes.to_vec())
}

fn unpack_tarball(pkg_dir: &PathBuf, data: Vec<u8>) -> Result<()> {
    let cursor = Cursor::new(data);
    let decoder = GzDecoder::new(cursor);
    let mut archive = Archive::new(decoder);

    archive
        .unpack(pkg_dir)
        .map_err(|source| SnpmError::Archive {
            path: pkg_dir.clone(),
            source,
        })?;

    Ok(())
}

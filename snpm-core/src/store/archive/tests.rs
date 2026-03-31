use super::paths::safe_join;
use super::unpack_tarball;

use flate2::Compression;
use flate2::write::GzEncoder;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use tar::{Builder, EntryType, Header};
use tempfile::tempdir;

fn build_tarball<F>(mut append: F) -> Vec<u8>
where
    F: FnMut(&mut Builder<Vec<u8>>),
{
    let mut builder = Builder::new(Vec::new());
    append(&mut builder);
    let tar_bytes = builder.into_inner().unwrap();

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&tar_bytes).unwrap();
    encoder.finish().unwrap()
}

#[test]
fn rejects_symlink_entry() {
    let bytes = build_tarball(|builder| {
        let mut header = Header::new_gnu();
        header.set_entry_type(EntryType::Symlink);
        header.set_path("package/symlink").unwrap();
        header.set_link_name("../../outside").unwrap();
        header.set_size(0);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, std::io::empty()).unwrap();
    });

    let temp = tempdir().unwrap();
    let result = unpack_tarball(temp.path(), bytes);
    assert!(result.is_err());
}

#[test]
fn rejects_hardlink_entry() {
    let bytes = build_tarball(|builder| {
        let mut header = Header::new_gnu();
        header.set_entry_type(EntryType::Link);
        header.set_path("package/link").unwrap();
        header.set_link_name("../../outside").unwrap();
        header.set_size(0);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, std::io::empty()).unwrap();
    });

    let temp = tempdir().unwrap();
    let result = unpack_tarball(temp.path(), bytes);
    assert!(result.is_err());
}

#[test]
fn unpack_tarball_extracts_files() {
    let bytes = build_tarball(|builder| {
        let content = b"console.log('hello');";
        let mut header = Header::new_gnu();
        header.set_entry_type(EntryType::Regular);
        header.set_path("package/index.js").unwrap();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder.append(&header, &content[..]).unwrap();
    });

    let temp = tempdir().unwrap();
    unpack_tarball(temp.path(), bytes).unwrap();

    let extracted = temp.path().join("package/index.js");
    assert!(extracted.is_file());
    assert_eq!(
        fs::read_to_string(&extracted).unwrap(),
        "console.log('hello');"
    );
}

#[test]
fn rejects_path_traversal_entry() {
    let bytes = build_tarball(|builder| {
        let content = b"malicious";
        let mut header = Header::new_gnu();
        header.set_entry_type(EntryType::Regular);
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        let name = b"../escape.txt";
        let gnu = header.as_gnu_mut().unwrap();
        gnu.name[..name.len()].copy_from_slice(name);
        header.set_cksum();
        builder.append(&header, &content[..]).unwrap();
    });

    let temp = tempdir().unwrap();
    let result = unpack_tarball(temp.path(), bytes);
    assert!(result.is_err());
}

#[test]
fn safe_join_normal() {
    let root = Path::new("/store");
    let rel = Path::new("package/index.js");
    assert_eq!(
        safe_join(root, rel),
        Some(PathBuf::from("/store/package/index.js"))
    );
}

#[test]
fn safe_join_rejects_parent_dir() {
    let root = Path::new("/store");
    let rel = Path::new("../escape");
    assert_eq!(safe_join(root, rel), None);
}

#[test]
fn safe_join_rejects_root_dir() {
    let root = Path::new("/store");
    let rel = Path::new("/etc/passwd");
    assert_eq!(safe_join(root, rel), None);
}

#[test]
fn safe_join_allows_curdir() {
    let root = Path::new("/store");
    let rel = Path::new("./package/file.js");
    assert_eq!(
        safe_join(root, rel),
        Some(PathBuf::from("/store/package/file.js"))
    );
}

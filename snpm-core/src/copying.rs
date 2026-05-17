use crate::linker::fs::capabilities::{record_reflink_outcome, reflink_likely};

use std::io;
use std::path::Path;

pub(crate) fn clone_or_copy_file(from: &Path, to: &Path) -> io::Result<()> {
    // Skip the syscall when an earlier reflink on the same (src_fs,
    // dst_fs) pair already failed — reflink_copy::reflink_or_copy
    // would otherwise attempt the clone, take an EXDEV / EOPNOTSUPP,
    // and then copy. Going straight to std::fs::copy saves a syscall
    // per file across thousands of files in a big install.
    if reflink_likely(from, to) {
        match reflink_copy::reflink(from, to) {
            Ok(()) => {
                record_reflink_outcome(from, to, true);
                return Ok(());
            }
            Err(_) => {
                record_reflink_outcome(from, to, false);
            }
        }
    }

    std::fs::copy(from, to).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::clone_or_copy_file;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn clone_or_copy_file_preserves_contents() {
        let dir = tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let destination = dir.path().join("destination.txt");

        fs::write(&source, "hello world").unwrap();
        clone_or_copy_file(&source, &destination).unwrap();

        assert_eq!(fs::read_to_string(&destination).unwrap(), "hello world");
    }

    #[cfg(unix)]
    #[test]
    fn clone_or_copy_file_does_not_alias_source_inode() {
        use std::os::unix::fs::MetadataExt;

        let dir = tempdir().unwrap();
        let source = dir.path().join("source.txt");
        let destination = dir.path().join("destination.txt");

        fs::write(&source, "hello world").unwrap();
        clone_or_copy_file(&source, &destination).unwrap();

        let source_inode = fs::metadata(&source).unwrap().ino();
        let destination_inode = fs::metadata(&destination).unwrap().ino();

        assert_ne!(source_inode, destination_inode);
    }
}

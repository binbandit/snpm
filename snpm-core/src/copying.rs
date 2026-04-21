use std::io;
use std::path::Path;

pub(crate) fn clone_or_copy_file(from: &Path, to: &Path) -> io::Result<()> {
    reflink_copy::reflink_or_copy(from, to).map(|_| ())
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

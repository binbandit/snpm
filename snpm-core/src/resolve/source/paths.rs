use std::path::{Path, PathBuf};

pub(in crate::resolve) fn resolve_relative_path(base: &Path, relative: &str) -> PathBuf {
    let mut components = base.to_path_buf();

    for part in Path::new(relative).components() {
        use std::path::Component;

        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                components.pop();
            }
            Component::Normal(component) => components.push(component),
            Component::RootDir => {
                components = PathBuf::from("/");
            }
            Component::Prefix(_) => {}
        }
    }

    components
}

mod apply;
mod diff;
mod session;
mod types;

pub use apply::{apply_patch, list_patches, materialize_patch_target, remove_patch};
pub use diff::create_patch;
pub use session::{
    cleanup_patch_session, find_installed_package, prepare_patch_directory, read_patch_session,
};
pub use types::{PatchInfo, PatchSession, get_patched_dependencies, parse_patch_key, patches_dir};

pub(super) const PATCHES_DIR: &str = "patches";
pub(super) const SESSION_MARKER: &str = ".snpm_patch_session";

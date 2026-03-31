mod command;
mod listing;
mod target;

pub use command::apply_patch;
pub use listing::{list_patches, remove_patch};
pub use target::materialize_patch_target;

#[cfg(test)]
mod tests;

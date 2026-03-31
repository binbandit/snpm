mod package;
mod version;

pub use package::RegistryPackage;
pub use version::{PeerDependencyMeta, RegistryDist, RegistryVersion};

#[cfg(test)]
mod tests;

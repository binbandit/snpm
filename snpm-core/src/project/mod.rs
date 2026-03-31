mod io;
mod manifest;

pub use io::Project;
pub use manifest::{
    BinField, CatalogMap, Manifest, ManifestPnpm, ManifestSnpm, NamedCatalogsMap, WorkspacesField,
};

#[cfg(test)]
mod tests;

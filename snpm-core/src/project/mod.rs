mod io;
mod manifest;

pub use io::Project;
pub use manifest::{
    BinField, CatalogMap, Manifest, ManifestPnpm, ManifestSnpm, ManifestSnpmPublish,
    NamedCatalogsMap, SourceMapPolicy, WorkspacesField,
};

#[cfg(test)]
mod tests;

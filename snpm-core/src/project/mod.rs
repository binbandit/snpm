mod io;
mod manifest;
mod package_json;

pub use io::Project;
pub use manifest::{
    BinField, CatalogMap, Manifest, ManifestPnpm, ManifestSnpm, ManifestSnpmPublish,
    NamedCatalogsMap, SourceMapPolicy, WorkspacesField,
};
pub use package_json::format_manifest;
pub(crate) use package_json::format_manifest_object;

#[cfg(test)]
mod tests;

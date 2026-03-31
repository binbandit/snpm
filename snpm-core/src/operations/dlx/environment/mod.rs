mod bin;
mod project;
mod resolution;

use crate::config::{LinkBackend, OfflineMode};
use crate::console;
use crate::{Result, SnpmConfig, SnpmError, http};

use std::collections::BTreeMap;
use std::path::PathBuf;
use tempfile::TempDir;

use bin::resolve_bin_path;
use project::temporary_project;
use resolution::resolve_dlx_graph;

use super::super::install::manifest::parse_spec;

pub(super) struct DlxEnvironment {
    pub(super) _temp_dir: TempDir,
    pub(super) bin_path: PathBuf,
}

pub(super) async fn prepare_dlx_environment(
    config: &SnpmConfig,
    package_spec: &str,
    offline_mode: OfflineMode,
) -> Result<DlxEnvironment> {
    let temp_dir = TempDir::new().map_err(|source| SnpmError::Io {
        path: PathBuf::from("temp_dlx"),
        source,
    })?;
    let temp_path = temp_dir.path().to_path_buf();

    console::verbose(&format!(
        "dlx: executing {} in temporary directory {}",
        package_spec,
        temp_path.display()
    ));

    let (name, range) = parse_spec(package_spec);
    let root_deps = BTreeMap::from([(name.clone(), range)]);
    let registry_client = http::create_client()?;

    console::step("Resolving package for dlx");

    let mut dlx_config = config.clone();
    dlx_config.link_backend = LinkBackend::Copy;

    let (graph, store_paths) = resolve_dlx_graph(
        &dlx_config,
        &registry_client,
        &root_deps,
        offline_mode,
        config,
    )
    .await?;

    console::step("Linking environment");

    let project = temporary_project(&temp_path, root_deps);
    crate::linker::link(&dlx_config, None, &project, &graph, &store_paths, false)?;

    Ok(DlxEnvironment {
        _temp_dir: temp_dir,
        bin_path: resolve_bin_path(&temp_path, &name)?,
    })
}

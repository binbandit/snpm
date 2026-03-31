mod manifest;
mod repo;
mod spec;

use crate::registry::RegistryPackage;
use crate::{Result, SnpmConfig};

use manifest::load_registry_package;
use repo::{prepare_repo, repo_cache_dir};
use spec::parse_git_spec;

pub async fn fetch_package(config: &SnpmConfig, url: &str) -> Result<RegistryPackage> {
    let git_spec = parse_git_spec(url)?;
    let cache_dir = repo_cache_dir(config, url);
    let repo_dir = prepare_repo(&cache_dir, &git_spec, url).await?;
    load_registry_package(&repo_dir, url)
}

use crate::{Project, Result, SnpmConfig};
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct InstallOptions {
    pub requested: Vec<String>,
}

pub async fn install(
    config: &SnpmConfig,
    project: &Project,
    options: InstallOptions,
) -> Result<()> {
    let mut planned = BTreeMap::new();

    for (name, version) in &project.manifest.dependencies {
        planned.insert(name.clone(), version.clone());
    }

    for name in options.requested {
        if !planned.contains_key(&name) {
            planned.insert(name, "latest".to_string());
        }
    }

    let _cache_dir = &config.cache_dir;
    let _data_dir = &config.data_dir;

    let _final_plan = planned;

    Ok(())
}

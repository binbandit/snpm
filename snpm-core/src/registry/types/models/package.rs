use serde::{Deserialize, Serialize};

use std::collections::BTreeMap;

use super::RegistryVersion;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegistryPackage {
    pub versions: BTreeMap<String, RegistryVersion>,
    #[serde(default)]
    pub time: BTreeMap<String, String>,
    #[serde(default, rename = "dist-tags")]
    pub dist_tags: BTreeMap<String, String>,
}

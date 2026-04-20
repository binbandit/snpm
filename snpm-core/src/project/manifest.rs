use serde::{Deserialize, Deserializer, Serialize};
use std::collections::BTreeMap;

pub type CatalogMap = BTreeMap<String, String>;
pub type NamedCatalogsMap = BTreeMap<String, BTreeMap<String, String>>;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum WorkspacesField {
    Patterns(Vec<String>),
    Object {
        packages: Vec<String>,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        catalog: CatalogMap,
        #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
        catalogs: NamedCatalogsMap,
    },
}

impl WorkspacesField {
    pub fn patterns(&self) -> &[String] {
        match self {
            WorkspacesField::Patterns(patterns) => patterns,
            WorkspacesField::Object { packages, .. } => packages,
        }
    }

    pub fn into_parts(self) -> (Vec<String>, CatalogMap, NamedCatalogsMap) {
        match self {
            WorkspacesField::Patterns(patterns) => (patterns, BTreeMap::new(), BTreeMap::new()),
            WorkspacesField::Object {
                packages,
                catalog,
                catalogs,
            } => (packages, catalog, catalogs),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub name: Option<String>,
    pub version: Option<String>,
    #[serde(default, deserialize_with = "deserialize_boolish")]
    pub private: bool,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    #[serde(default)]
    pub dev_dependencies: BTreeMap<String, String>,
    #[serde(default)]
    pub optional_dependencies: BTreeMap<String, String>,
    #[serde(default)]
    pub scripts: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub files: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bin: Option<BinField>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub main: Option<String>,
    #[serde(default)]
    pub pnpm: Option<ManifestPnpm>,
    #[serde(default)]
    pub snpm: Option<ManifestSnpm>,
    #[serde(default)]
    pub workspaces: Option<WorkspacesField>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BinField {
    Single(String),
    Map(BTreeMap<String, String>),
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestPnpm {
    #[serde(default)]
    pub overrides: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patched_dependencies: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSnpm {
    #[serde(default)]
    pub overrides: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patched_dependencies: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publish: Option<ManifestSnpmPublish>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestSnpmPublish {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub deny: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allow_risks: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_files: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_bytes: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_maps: Option<SourceMapPolicy>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SourceMapPolicy {
    Forbid,
    ExternalOnly,
    Allow,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum BoolishValue {
    Bool(bool),
    String(String),
}

fn deserialize_boolish<'de, D>(deserializer: D) -> std::result::Result<bool, D::Error>
where
    D: Deserializer<'de>,
{
    let value = Option::<BoolishValue>::deserialize(deserializer)?;

    match value {
        None => Ok(false),
        Some(BoolishValue::Bool(value)) => Ok(value),
        Some(BoolishValue::String(value)) => match value.trim().to_ascii_lowercase().as_str() {
            "true" => Ok(true),
            "false" => Ok(false),
            other => Err(serde::de::Error::custom(format!(
                "expected a boolean, got {other}"
            ))),
        },
    }
}

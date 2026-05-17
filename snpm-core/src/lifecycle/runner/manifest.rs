use crate::project::BinField;
use crate::{Result, SnpmError};

use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub(super) fn read_manifest(manifest_path: &Path) -> Result<Value> {
    let data = fs::read_to_string(manifest_path).map_err(|source| SnpmError::ReadFile {
        path: manifest_path.to_path_buf(),
        source,
    })?;

    serde_json::from_str(&data).map_err(|source| SnpmError::ParseJson {
        path: manifest_path.to_path_buf(),
        source,
    })
}

pub(super) fn package_name(value: &Value) -> Option<&str> {
    value.get("name").and_then(|name| name.as_str())
}

pub(super) fn package_version(value: &Value) -> Option<&str> {
    value.get("version").and_then(|version| version.as_str())
}

pub(super) fn package_scripts(value: &Value) -> Option<&serde_json::Map<String, Value>> {
    match value.get("scripts") {
        Some(Value::Object(map)) => Some(map),
        _ => None,
    }
}

pub(super) fn package_bin(value: &Value) -> Option<BinField> {
    let bin = value.get("bin")?;
    match bin {
        Value::String(script) if !script.is_empty() => Some(BinField::Single(script.clone())),
        Value::Object(map) => {
            let entries: BTreeMap<String, String> = map
                .iter()
                .filter_map(|(name, value)| {
                    value.as_str().map(|spec| (name.clone(), spec.to_string()))
                })
                .collect();
            if entries.is_empty() {
                None
            } else {
                Some(BinField::Map(entries))
            }
        }
        _ => None,
    }
}

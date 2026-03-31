use crate::Project;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PatchSession {
    pub package_name: String,
    pub package_version: String,
    pub original_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct PatchInfo {
    pub package_name: String,
    pub package_version: String,
    pub patch_path: PathBuf,
}

pub fn patches_dir(project: &Project) -> PathBuf {
    project.root.join(super::PATCHES_DIR)
}

pub fn parse_patch_key(key: &str) -> Option<(String, String)> {
    let at_pos = key.rfind('@')?;
    if at_pos == 0 {
        return None;
    }

    Some((key[..at_pos].to_string(), key[at_pos + 1..].to_string()))
}

pub fn get_patched_dependencies(project: &Project) -> BTreeMap<String, String> {
    let mut patched = BTreeMap::new();

    if let Some(ref pnpm) = project.manifest.pnpm
        && let Some(ref deps) = pnpm.patched_dependencies
    {
        patched.extend(deps.clone());
    }

    if let Some(ref snpm) = project.manifest.snpm
        && let Some(ref deps) = snpm.patched_dependencies
    {
        patched.extend(deps.clone());
    }

    patched
}

#[cfg(test)]
mod tests {
    use super::parse_patch_key;

    #[test]
    fn parse_patch_key_simple() {
        let result = parse_patch_key("lodash@4.17.21");
        assert_eq!(result, Some(("lodash".to_string(), "4.17.21".to_string())));
    }

    #[test]
    fn parse_patch_key_scoped() {
        let result = parse_patch_key("@types/node@18.0.0");
        assert_eq!(
            result,
            Some(("@types/node".to_string(), "18.0.0".to_string()))
        );
    }

    #[test]
    fn parse_patch_key_no_at() {
        assert!(parse_patch_key("lodash").is_none());
    }

    #[test]
    fn parse_patch_key_only_at_start() {
        assert!(parse_patch_key("@scope").is_none());
    }
}

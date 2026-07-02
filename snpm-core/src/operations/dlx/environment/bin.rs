use crate::console;
use crate::{Result, SnpmError};

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub(super) fn resolve_bin_path(temp_path: &Path, package_name: &str) -> Result<PathBuf> {
    let bin_name = package_name.rsplit('/').next().unwrap_or(package_name);
    let bin_dir = temp_path.join("node_modules").join(".bin");
    let bin_path = bin_dir.join(bin_name);

    if bin_path.exists() {
        return Ok(bin_path);
    }

    // The name-derived launcher doesn't exist (e.g. `typescript` ships
    // `tsc`/`tsserver`). Consult the package's own bin map so the
    // choice is deterministic instead of depending on directory order.
    if let Some(manifest_bins) = read_manifest_bin_names(temp_path, package_name) {
        for candidate in &manifest_bins {
            let candidate_path = bin_dir.join(candidate);
            if candidate_path.exists() {
                if manifest_bins.len() > 1 {
                    console::verbose(&format!(
                        "Default binary {} not found; using {} (first of {:?} from the package's bin map)",
                        bin_name, candidate, manifest_bins
                    ));
                }
                return Ok(candidate_path);
            }
        }
    }

    // Last resort: pick the alphabetically-first launcher so behavior
    // is at least stable across runs and filesystems.
    let mut launchers: Vec<PathBuf> = std::fs::read_dir(&bin_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_file() || path.is_symlink())
        .collect();
    launchers.sort();

    if let Some(path) = launchers.into_iter().next() {
        console::verbose(&format!(
            "Default binary {} not found, utilizing {}",
            bin_name,
            path.display()
        ));
        return Ok(path);
    }

    Err(SnpmError::ScriptRun {
        name: package_name.to_string(),
        reason: "Binary not found".to_string(),
    })
}

/// Bin names declared by the dlx'd package itself, sorted for
/// determinism. `None` when the manifest is missing or has no bin map.
fn read_manifest_bin_names(temp_path: &Path, package_name: &str) -> Option<Vec<String>> {
    let manifest_path = temp_path
        .join("node_modules")
        .join(package_name)
        .join("package.json");
    let content = std::fs::read_to_string(manifest_path).ok()?;
    let manifest: serde_json::Value = serde_json::from_str(&content).ok()?;

    match manifest.get("bin")? {
        serde_json::Value::String(_) => {
            // Single-string bin is named after the package; that case
            // was already covered by the name-derived lookup.
            None
        }
        serde_json::Value::Object(map) => {
            let names: BTreeMap<&String, ()> = map.iter().map(|(name, _)| (name, ())).collect();
            let sorted: Vec<String> = names.keys().map(|name| (*name).clone()).collect();
            (!sorted.is_empty()).then_some(sorted)
        }
        _ => None,
    }
}

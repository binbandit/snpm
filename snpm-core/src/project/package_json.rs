use super::Manifest;
use crate::{Result, SnpmError};
use serde::Serialize;
use serde::ser::SerializeMap;
use serde_json::Value;
use std::path::Path;

const PACKAGE_JSON_FIELD_ORDER: &[&str] = &[
    "$schema",
    "name",
    "displayName",
    "version",
    "stableVersion",
    "private",
    "description",
    "categories",
    "keywords",
    "homepage",
    "bugs",
    "repository",
    "funding",
    "license",
    "qna",
    "author",
    "maintainers",
    "contributors",
    "publisher",
    "sideEffects",
    "type",
    "imports",
    "exports",
    "main",
    "svelte",
    "umd:main",
    "jsdelivr",
    "unpkg",
    "module",
    "source",
    "jsnext:main",
    "browser",
    "react-native",
    "types",
    "typesVersions",
    "typings",
    "style",
    "example",
    "examplestyle",
    "assets",
    "bin",
    "man",
    "directories",
    "files",
    "workspaces",
    "binary",
    "scripts",
    "betterScripts",
    "l10n",
    "contributes",
    "activationEvents",
    "husky",
    "simple-git-hooks",
    "pre-commit",
    "commitlint",
    "lint-staged",
    "nano-staged",
    "config",
    "nodemonConfig",
    "browserify",
    "babel",
    "browserslist",
    "xo",
    "prettier",
    "eslintConfig",
    "eslintIgnore",
    "npmpkgjsonlint",
    "npmPackageJsonLintConfig",
    "npmpackagejsonlint",
    "release",
    "remarkConfig",
    "stylelint",
    "ava",
    "jest",
    "jest-junit",
    "jest-stare",
    "mocha",
    "nyc",
    "c8",
    "tap",
    "oclif",
    "resolutions",
    "overrides",
    "dependencies",
    "devDependencies",
    "dependenciesMeta",
    "peerDependencies",
    "peerDependenciesMeta",
    "optionalDependencies",
    "bundledDependencies",
    "bundleDependencies",
    "extensionPack",
    "extensionDependencies",
    "flat",
    "packageManager",
    "engines",
    "engineStrict",
    "devEngines",
    "volta",
    "languageName",
    "os",
    "cpu",
    "preferGlobal",
    "publishConfig",
    "icon",
    "badges",
    "galleryBanner",
    "preview",
    "markdown",
    "pnpm",
    "snpm",
];

pub fn format_manifest(manifest: &Manifest, path: &Path) -> Result<String> {
    let value = serde_json::to_value(manifest).map_err(|error| SnpmError::SerializeJson {
        path: path.to_path_buf(),
        reason: error.to_string(),
    })?;

    let Value::Object(object) = value else {
        return Err(SnpmError::SerializeJson {
            path: path.to_path_buf(),
            reason: "manifest did not serialize to a JSON object".to_string(),
        });
    };

    let mut data = serde_json::to_string_pretty(&OrderedPackageJson::from_object(object)).map_err(
        |error| SnpmError::SerializeJson {
            path: path.to_path_buf(),
            reason: error.to_string(),
        },
    )?;
    data.push('\n');

    Ok(data)
}

struct OrderedPackageJson {
    entries: Vec<(String, Value)>,
}

impl OrderedPackageJson {
    fn from_object(mut object: serde_json::Map<String, Value>) -> Self {
        let mut entries = Vec::new();

        for key in PACKAGE_JSON_FIELD_ORDER {
            if let Some(value) = object.remove(*key) {
                entries.push(((*key).to_string(), value));
            }
        }

        let mut remaining = object.into_iter().collect::<Vec<_>>();
        remaining.sort_by(|(left, _), (right, _)| left.cmp(right));
        entries.extend(remaining);

        Self { entries }
    }
}

impl Serialize for OrderedPackageJson {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut map = serializer.serialize_map(Some(self.entries.len()))?;

        for (key, value) in &self.entries {
            map.serialize_entry(key, value)?;
        }

        map.end()
    }
}

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegistryPackage {
    pub versions: BTreeMap<String, RegistryVersion>,
    #[serde(default)]
    pub time: BTreeMap<String, serde_json::Value>,
    #[serde(default, rename = "dist-tags")]
    pub dist_tags: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PeerDependencyMeta {
    #[serde(default)]
    pub optional: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(untagged)]
pub enum BundledDependencies {
    List(Vec<String>),
    All(bool),
}

impl BundledDependencies {
    pub fn to_set(&self, all_deps: &BTreeMap<String, String>) -> BTreeSet<String> {
        match self {
            BundledDependencies::List(list) => list.iter().cloned().collect(),
            BundledDependencies::All(true) => all_deps.keys().cloned().collect(),
            BundledDependencies::All(false) => BTreeSet::new(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            BundledDependencies::List(list) => list.is_empty(),
            BundledDependencies::All(val) => !val,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegistryVersion {
    pub version: String,
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
    #[serde(default, rename = "optionalDependencies")]
    pub optional_dependencies: BTreeMap<String, String>,
    #[serde(default, rename = "peerDependencies")]
    pub peer_dependencies: BTreeMap<String, String>,
    #[serde(default, rename = "peerDependenciesMeta")]
    pub peer_dependencies_meta: BTreeMap<String, PeerDependencyMeta>,
    #[serde(default, rename = "bundledDependencies")]
    pub bundled_dependencies: Option<BundledDependencies>,
    #[serde(default, rename = "bundleDependencies")]
    pub bundle_dependencies: Option<BundledDependencies>,
    pub dist: RegistryDist,
    #[serde(default)]
    pub os: Vec<String>,
    #[serde(default)]
    pub cpu: Vec<String>,
    #[serde(default)]
    pub bin: Option<serde_json::Value>,
}

impl RegistryVersion {
    pub fn get_bundled_dependencies(&self) -> Option<&BundledDependencies> {
        self.bundled_dependencies
            .as_ref()
            .or(self.bundle_dependencies.as_ref())
    }

    pub fn has_bin(&self) -> bool {
        self.bin.as_ref().map(|b| !b.is_null()).unwrap_or(false)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct RegistryProtocol {
    pub name: String,
}

impl RegistryProtocol {
    pub fn npm() -> Self {
        RegistryProtocol {
            name: "npm".to_string(),
        }
    }

    pub fn git() -> Self {
        RegistryProtocol {
            name: "git".to_string(),
        }
    }

    pub fn jsr() -> Self {
        RegistryProtocol {
            name: "jsr".to_string(),
        }
    }

    pub fn file() -> Self {
        RegistryProtocol {
            name: "file".to_string(),
        }
    }

    pub fn custom(name: &str) -> Self {
        RegistryProtocol {
            name: name.to_string(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegistryDist {
    pub tarball: String,
    #[serde(default)]
    pub integrity: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bundled_list_to_set() {
        let deps = BTreeMap::from([
            ("a".to_string(), "1.0.0".to_string()),
            ("b".to_string(), "2.0.0".to_string()),
        ]);
        let bundled = BundledDependencies::List(vec!["a".to_string()]);
        let set = bundled.to_set(&deps);
        assert!(set.contains("a"));
        assert!(!set.contains("b"));
    }

    #[test]
    fn bundled_all_true_to_set() {
        let deps = BTreeMap::from([
            ("a".to_string(), "1.0.0".to_string()),
            ("b".to_string(), "2.0.0".to_string()),
        ]);
        let bundled = BundledDependencies::All(true);
        let set = bundled.to_set(&deps);
        assert!(set.contains("a"));
        assert!(set.contains("b"));
    }

    #[test]
    fn bundled_all_false_to_set() {
        let deps = BTreeMap::from([("a".to_string(), "1.0.0".to_string())]);
        let bundled = BundledDependencies::All(false);
        let set = bundled.to_set(&deps);
        assert!(set.is_empty());
    }

    #[test]
    fn bundled_list_is_empty() {
        assert!(BundledDependencies::List(vec![]).is_empty());
        assert!(!BundledDependencies::List(vec!["a".to_string()]).is_empty());
    }

    #[test]
    fn bundled_all_is_empty() {
        assert!(!BundledDependencies::All(true).is_empty());
        assert!(BundledDependencies::All(false).is_empty());
    }

    #[test]
    fn registry_version_has_bin_string() {
        let v = RegistryVersion {
            version: "1.0.0".to_string(),
            dependencies: BTreeMap::new(),
            optional_dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            peer_dependencies_meta: BTreeMap::new(),
            bundled_dependencies: None,
            bundle_dependencies: None,
            dist: RegistryDist {
                tarball: "url".to_string(),
                integrity: None,
            },
            os: vec![],
            cpu: vec![],
            bin: Some(serde_json::json!("./cli.js")),
        };
        assert!(v.has_bin());
    }

    #[test]
    fn registry_version_has_bin_null() {
        let v = RegistryVersion {
            version: "1.0.0".to_string(),
            dependencies: BTreeMap::new(),
            optional_dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            peer_dependencies_meta: BTreeMap::new(),
            bundled_dependencies: None,
            bundle_dependencies: None,
            dist: RegistryDist {
                tarball: "url".to_string(),
                integrity: None,
            },
            os: vec![],
            cpu: vec![],
            bin: Some(serde_json::json!(null)),
        };
        assert!(!v.has_bin());
    }

    #[test]
    fn registry_version_has_bin_none() {
        let v = RegistryVersion {
            version: "1.0.0".to_string(),
            dependencies: BTreeMap::new(),
            optional_dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            peer_dependencies_meta: BTreeMap::new(),
            bundled_dependencies: None,
            bundle_dependencies: None,
            dist: RegistryDist {
                tarball: "url".to_string(),
                integrity: None,
            },
            os: vec![],
            cpu: vec![],
            bin: None,
        };
        assert!(!v.has_bin());
    }

    #[test]
    fn registry_version_get_bundled_prefers_bundled_dependencies() {
        let v = RegistryVersion {
            version: "1.0.0".to_string(),
            dependencies: BTreeMap::new(),
            optional_dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            peer_dependencies_meta: BTreeMap::new(),
            bundled_dependencies: Some(BundledDependencies::List(vec!["a".to_string()])),
            bundle_dependencies: Some(BundledDependencies::List(vec!["b".to_string()])),
            dist: RegistryDist {
                tarball: "url".to_string(),
                integrity: None,
            },
            os: vec![],
            cpu: vec![],
            bin: None,
        };
        let bundled = v.get_bundled_dependencies().unwrap();
        match bundled {
            BundledDependencies::List(list) => assert_eq!(list, &vec!["a".to_string()]),
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn registry_version_get_bundled_falls_back_to_bundle() {
        let v = RegistryVersion {
            version: "1.0.0".to_string(),
            dependencies: BTreeMap::new(),
            optional_dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            peer_dependencies_meta: BTreeMap::new(),
            bundled_dependencies: None,
            bundle_dependencies: Some(BundledDependencies::List(vec!["b".to_string()])),
            dist: RegistryDist {
                tarball: "url".to_string(),
                integrity: None,
            },
            os: vec![],
            cpu: vec![],
            bin: None,
        };
        let bundled = v.get_bundled_dependencies().unwrap();
        match bundled {
            BundledDependencies::List(list) => assert_eq!(list, &vec!["b".to_string()]),
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn registry_protocol_equality() {
        assert_eq!(RegistryProtocol::npm(), RegistryProtocol::npm());
        assert_ne!(RegistryProtocol::npm(), RegistryProtocol::git());
        assert_eq!(
            RegistryProtocol::custom("test"),
            RegistryProtocol::custom("test")
        );
    }

    #[test]
    fn registry_package_deserializes_from_json() {
        let json = r#"{
            "versions": {
                "1.0.0": {
                    "version": "1.0.0",
                    "dist": { "tarball": "https://example.com/pkg.tgz" }
                }
            },
            "dist-tags": { "latest": "1.0.0" }
        }"#;
        let pkg: RegistryPackage = serde_json::from_str(json).unwrap();
        assert!(pkg.versions.contains_key("1.0.0"));
        assert_eq!(
            pkg.dist_tags.get("latest").map(String::as_str),
            Some("1.0.0")
        );
    }

    #[test]
    fn bundled_dependencies_deserializes_list() {
        let json = r#"["dep-a", "dep-b"]"#;
        let bundled: BundledDependencies = serde_json::from_str(json).unwrap();
        match bundled {
            BundledDependencies::List(list) => assert_eq!(list.len(), 2),
            _ => panic!("expected List"),
        }
    }

    #[test]
    fn bundled_dependencies_deserializes_bool() {
        let json = "true";
        let bundled: BundledDependencies = serde_json::from_str(json).unwrap();
        assert!(matches!(bundled, BundledDependencies::All(true)));
    }
}

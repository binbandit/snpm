use serde::{Deserialize, Serialize};

use std::collections::{BTreeMap, BTreeSet};

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
            BundledDependencies::All(value) => !value,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BundledDependencies;

    use std::collections::BTreeMap;

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

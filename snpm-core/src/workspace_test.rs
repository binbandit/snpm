#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn test_catalog_overlay() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("snpm_test_{}", timestamp));
        fs::create_dir_all(&dir).unwrap();

        let workspace_yaml = r#"
packages: []
catalog:
  react: "18.0.0"
  lodash: "4.0.0"
"#;
        fs::write(dir.join("snpm-workspace.yaml"), workspace_yaml).unwrap();

        let catalog_yaml = r#"
catalog:
  react: "17.0.0" # Should be ignored (workspace takes precedence)
  axios: "1.0.0"  # Should be added
"#;
        fs::write(dir.join("snpm-catalog.yaml"), catalog_yaml).unwrap();

        let workspace = Workspace::discover(&dir).unwrap().unwrap();

        assert_eq!(
            workspace.config.catalog.get("react").map(|s| s.as_str()),
            Some("18.0.0")
        );
        assert_eq!(
            workspace.config.catalog.get("lodash").map(|s| s.as_str()),
            Some("4.0.0")
        );
        assert_eq!(
            workspace.config.catalog.get("axios").map(|s| s.as_str()),
            Some("1.0.0")
        );

        fs::remove_dir_all(&dir).unwrap();
    }
}

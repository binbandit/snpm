use crate::SnpmConfig;

use std::path::PathBuf;

pub(in crate::cache) fn sanitize_package_name(name: &str) -> String {
    name.replace('/', "__")
}

pub(in crate::cache) fn package_cache_dir(config: &SnpmConfig, name: &str) -> PathBuf {
    config.metadata_dir().join(sanitize_package_name(name))
}

pub(in crate::cache) fn metadata_cache_path(config: &SnpmConfig, name: &str) -> PathBuf {
    package_cache_dir(config, name).join("index.json")
}

pub(in crate::cache) fn headers_cache_path(config: &SnpmConfig, name: &str) -> PathBuf {
    package_cache_dir(config, name).join("headers.json")
}

#[cfg(test)]
mod tests {
    use super::sanitize_package_name;

    #[test]
    fn sanitize_package_name_simple() {
        assert_eq!(sanitize_package_name("lodash"), "lodash");
    }

    #[test]
    fn sanitize_package_name_scoped() {
        assert_eq!(sanitize_package_name("@types/node"), "@types__node");
    }

    #[test]
    fn sanitize_package_name_multiple_slashes() {
        assert_eq!(sanitize_package_name("a/b/c"), "a__b__c");
    }
}

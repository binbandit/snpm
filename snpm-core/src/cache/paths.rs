use crate::SnpmConfig;

use std::path::PathBuf;

const PACKUMENT_CACHE_SHARDS: u32 = 64;

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

pub(in crate::cache) fn metadata_shard_path(config: &SnpmConfig, name: &str) -> PathBuf {
    let shard = shard_for_name(name);
    config
        .metadata_dir()
        .join(format!("packuments-v1-{shard:02x}.bin"))
}

fn shard_for_name(name: &str) -> u32 {
    let mut hash = 2_166_136_261u32;
    for byte in name.as_bytes() {
        hash ^= u32::from(*byte);
        hash = hash.wrapping_mul(16_777_619);
    }

    hash % PACKUMENT_CACHE_SHARDS
}

#[cfg(test)]
mod tests {
    use super::{sanitize_package_name, shard_for_name};

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

    #[test]
    fn shard_for_name_is_stable() {
        assert_eq!(shard_for_name("lodash"), shard_for_name("lodash"));
        assert_ne!(shard_for_name("lodash"), shard_for_name("left-pad"));
    }
}

use crate::{Result, SnpmError};

use std::env;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeArchive {
    TarGz,
    Zip,
}

impl NodeArchive {
    pub fn extension(self) -> &'static str {
        match self {
            NodeArchive::TarGz => "tar.gz",
            NodeArchive::Zip => "zip",
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodePlatform {
    pub os: String,
    pub arch: String,
    pub archive: NodeArchive,
}

impl NodePlatform {
    pub fn detect() -> Result<Self> {
        let os = node_os(env::consts::OS).ok_or_else(|| SnpmError::Internal {
            reason: format!("unsupported OS for Node downloads: {}", env::consts::OS),
        })?;

        let arch = node_arch(env::consts::ARCH).ok_or_else(|| SnpmError::Internal {
            reason: format!(
                "unsupported architecture for Node downloads: {}",
                env::consts::ARCH
            ),
        })?;

        let archive = if os == "win" {
            NodeArchive::Zip
        } else {
            NodeArchive::TarGz
        };

        Ok(NodePlatform {
            os: os.to_string(),
            arch: arch.to_string(),
            archive,
        })
    }

    pub fn asset_stem(&self, version_with_v: &str) -> String {
        format!("node-{version_with_v}-{}-{}", self.os, self.arch)
    }

    pub fn asset_filename(&self, version_with_v: &str) -> String {
        format!(
            "{}.{}",
            self.asset_stem(version_with_v),
            self.archive.extension()
        )
    }
}

fn node_os(host: &str) -> Option<&'static str> {
    match host {
        "macos" => Some("darwin"),
        "linux" => Some("linux"),
        "windows" => Some("win"),
        // No "freebsd" mapping: nodejs.org ships no FreeBSD builds, and
        // silently downloading Linux binaries there would not run.
        _ => None,
    }
}

fn node_arch(host: &str) -> Option<&'static str> {
    match host {
        "x86_64" => Some("x64"),
        "aarch64" => Some("arm64"),
        "x86" => Some("x86"),
        "arm" => Some("armv7l"),
        "powerpc64" => Some("ppc64le"),
        "s390x" => Some("s390x"),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{NodeArchive, NodePlatform, node_arch, node_os};

    #[test]
    fn maps_common_os_strings() {
        assert_eq!(node_os("macos"), Some("darwin"));
        assert_eq!(node_os("linux"), Some("linux"));
        assert_eq!(node_os("windows"), Some("win"));
        assert!(node_os("haiku").is_none());
    }

    #[test]
    fn maps_common_arch_strings() {
        assert_eq!(node_arch("x86_64"), Some("x64"));
        assert_eq!(node_arch("aarch64"), Some("arm64"));
        assert!(node_arch("riscv64").is_none());
    }

    #[test]
    fn asset_filename_matches_node_dist_layout() {
        let platform = NodePlatform {
            os: "darwin".into(),
            arch: "arm64".into(),
            archive: NodeArchive::TarGz,
        };
        assert_eq!(
            platform.asset_filename("v20.10.0"),
            "node-v20.10.0-darwin-arm64.tar.gz"
        );
    }

    #[test]
    fn asset_filename_uses_zip_for_windows() {
        let platform = NodePlatform {
            os: "win".into(),
            arch: "x64".into(),
            archive: NodeArchive::Zip,
        };
        assert_eq!(
            platform.asset_filename("v20.10.0"),
            "node-v20.10.0-win-x64.zip"
        );
    }
}

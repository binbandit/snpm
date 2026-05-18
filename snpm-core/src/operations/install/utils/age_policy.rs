use crate::config::OfflineMode;
use crate::registry::RegistryProtocol;
use crate::resolve::{ResolutionGraph, ResolvedPackage};
use crate::version::version_age_days;
use crate::{Result, SnpmConfig, SnpmError};
use futures::stream::{self, StreamExt, TryStreamExt};
use std::collections::{BTreeMap, BTreeSet};
use time::OffsetDateTime;

pub(crate) async fn validate_graph_min_package_age(
    config: &SnpmConfig,
    client: &reqwest::Client,
    graph: &ResolutionGraph,
    force: bool,
) -> Result<()> {
    let Some(min_days) = config.min_package_age_days else {
        return Ok(());
    };
    if force {
        return Ok(());
    }

    let mut package_versions: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    for package in graph
        .packages
        .values()
        .filter(|package| should_check_registry_age(package))
    {
        let protocol = age_check_protocol(package);
        package_versions
            .entry((protocol.name, package.id.name.clone()))
            .or_default()
            .insert(package.id.version.clone());
    }

    if package_versions.is_empty() {
        return Ok(());
    }

    let concurrency = crate::store::registry_task_concurrency(config);
    stream::iter(package_versions)
        .map(|((protocol_name, name), versions)| async move {
            let protocol = RegistryProtocol::custom(&protocol_name);
            validate_package_versions_min_age(config, client, &protocol, &name, &versions, min_days)
                .await
        })
        .buffer_unordered(concurrency)
        .try_collect::<Vec<_>>()
        .await?;

    Ok(())
}

async fn validate_package_versions_min_age(
    config: &SnpmConfig,
    client: &reqwest::Client,
    protocol: &RegistryProtocol,
    name: &str,
    versions: &BTreeSet<String>,
    min_days: u32,
) -> Result<()> {
    let registry_package = crate::registry::fetch_package_with_offline(
        config,
        client,
        name,
        protocol,
        OfflineMode::Online,
    )
    .await?;

    let now = OffsetDateTime::now_utc();
    for version in versions {
        let Some(age_days) = version_age_days(&registry_package, version, now) else {
            continue;
        };

        if age_days >= min_days as i64 {
            continue;
        }

        return Err(SnpmError::ResolutionFailed {
            name: name.to_string(),
            range: version.clone(),
            reason: format!(
                "locked version {} is only {} days old, less than the configured minimum of {} days",
                version, age_days, min_days
            ),
        });
    }

    Ok(())
}

fn should_check_registry_age(package: &ResolvedPackage) -> bool {
    package.tarball.starts_with("http://") || package.tarball.starts_with("https://")
}

fn age_check_protocol(package: &ResolvedPackage) -> RegistryProtocol {
    if package.tarball.contains("npm.jsr.io/") {
        RegistryProtocol::jsr()
    } else {
        RegistryProtocol::npm()
    }
}

#[cfg(test)]
mod tests {
    use super::{age_check_protocol, should_check_registry_age, validate_graph_min_package_age};
    use crate::config::{AuthScheme, HoistingMode, LinkBackend, SnpmConfig};
    use crate::resolve::{
        PackageId, ResolutionGraph, ResolutionRoot, ResolvedPackage, RootDependency,
    };
    use std::collections::{BTreeMap, BTreeSet};
    use std::path::PathBuf;
    use time::format_description::well_known::Rfc3339;
    use time::{Duration, OffsetDateTime};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn package_with_tarball(tarball: &str) -> ResolvedPackage {
        ResolvedPackage {
            id: PackageId {
                name: "pkg".to_string(),
                version: "1.0.0".to_string(),
            },
            tarball: tarball.to_string(),
            integrity: None,
            dependencies: BTreeMap::new(),
            peer_dependencies: BTreeMap::new(),
            bundled_dependencies: None,
            has_bin: false,
            bin: None,
        }
    }

    #[test]
    fn age_policy_checks_registry_tarballs_only() {
        assert!(should_check_registry_age(&package_with_tarball(
            "https://registry.npmjs.org/pkg/-/pkg-1.0.0.tgz"
        )));
        assert!(should_check_registry_age(&package_with_tarball(
            "http://registry.example/pkg/-/pkg-1.0.0.tgz"
        )));
        assert!(!should_check_registry_age(&package_with_tarball(
            "file:///tmp/pkg"
        )));
        assert!(!should_check_registry_age(&package_with_tarball("")));
    }

    #[test]
    fn age_policy_infers_jsr_protocol_from_tarball_host() {
        assert_eq!(
            age_check_protocol(&package_with_tarball(
                "https://npm.jsr.io/@jsr/std__assert/-/assert-1.0.0.tgz"
            )),
            crate::registry::RegistryProtocol::jsr()
        );
        assert_eq!(
            age_check_protocol(&package_with_tarball(
                "https://registry.npmjs.org/pkg/-/pkg-1.0.0.tgz"
            )),
            crate::registry::RegistryProtocol::npm()
        );
    }

    fn make_config(registry: String) -> SnpmConfig {
        SnpmConfig {
            cache_dir: PathBuf::from("/tmp/cache"),
            data_dir: PathBuf::from("/tmp/data"),
            allow_scripts: BTreeSet::new(),
            disable_global_virtual_store_for_packages: BTreeSet::new(),
            min_package_age_days: Some(7),
            min_package_cache_age_days: None,
            default_registry: registry,
            scoped_registries: BTreeMap::new(),
            registry_auth: BTreeMap::new(),
            default_registry_auth_token: None,
            default_registry_auth_scheme: AuthScheme::Bearer,
            registry_auth_schemes: BTreeMap::new(),
            hoisting: HoistingMode::SingleVersion,
            link_backend: LinkBackend::Auto,
            strict_peers: false,
            frozen_lockfile_default: false,
            always_auth: false,
            registry_concurrency: 4,
            verbose: false,
            log_file: None,
            remote_cache_url: None,
            remote_cache_auth_token: None,
            remote_cache_read_only: false,
        }
    }

    fn graph_with_registry_package(tarball: String) -> ResolutionGraph {
        let id = PackageId {
            name: "pkg".to_string(),
            version: "1.0.0".to_string(),
        };
        ResolutionGraph {
            root: ResolutionRoot {
                dependencies: BTreeMap::from([(
                    "pkg".to_string(),
                    RootDependency {
                        requested: "^1.0.0".to_string(),
                        resolved: id.clone(),
                    },
                )]),
            },
            packages: BTreeMap::from([(
                id.clone(),
                ResolvedPackage {
                    id,
                    tarball,
                    integrity: None,
                    dependencies: BTreeMap::new(),
                    peer_dependencies: BTreeMap::new(),
                    bundled_dependencies: None,
                    has_bin: false,
                    bin: None,
                },
            )]),
        }
    }

    async fn serve_packument(published_days_ago: i64) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = vec![0; 2048];
            let _ = socket.read(&mut request).await.unwrap();
            let published_at = (OffsetDateTime::now_utc() - Duration::days(published_days_ago))
                .format(&Rfc3339)
                .unwrap();
            let body = format!(
                r#"{{
  "versions": {{
    "1.0.0": {{
      "version": "1.0.0",
      "dist": {{ "tarball": "http://{addr}/pkg/-/pkg-1.0.0.tgz" }}
    }}
  }},
  "time": {{ "1.0.0": "{published_at}" }},
  "dist-tags": {{ "latest": "1.0.0" }}
}}"#
            );
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn graph_age_policy_rejects_young_locked_registry_package() {
        let registry = serve_packument(1).await;
        let config = make_config(registry.clone());
        let client = reqwest::Client::new();
        let graph = graph_with_registry_package(format!("{registry}/pkg/-/pkg-1.0.0.tgz"));

        let error = validate_graph_min_package_age(&config, &client, &graph, false)
            .await
            .expect_err("young locked package should fail age policy");

        assert!(error.to_string().contains("locked version 1.0.0"));
    }
}

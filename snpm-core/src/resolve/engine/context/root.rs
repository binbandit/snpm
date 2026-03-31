use super::super::super::types::{ResolutionRoot, RootDependency};
use super::ResolverContext;
use crate::registry::RegistryProtocol;
use crate::{Result, SnpmError, console};

use futures::future::join_all;
use std::collections::{BTreeMap, BTreeSet};

impl<'a> ResolverContext<'a> {
    pub(in crate::resolve::engine) async fn resolve_root_dependencies(
        &self,
        root_deps: &BTreeMap<String, String>,
        root_protocols: &BTreeMap<String, RegistryProtocol>,
        optional_root_names: &BTreeSet<String>,
    ) -> Result<ResolutionRoot> {
        let default_protocol = RegistryProtocol::npm();
        let mut tasks = Vec::new();

        for (name, range) in root_deps {
            let context = self.clone();
            let name = name.clone();
            let range = range.clone();
            let protocol = root_protocols
                .get(&name)
                .unwrap_or(&default_protocol)
                .clone();
            let is_optional = optional_root_names.contains(&name);

            tasks.push(async move {
                match context.resolve_package(&name, &range, &protocol).await {
                    Ok(id) => Ok::<Option<(String, RootDependency)>, SnpmError>(Some((
                        name,
                        RootDependency {
                            requested: range,
                            resolved: id,
                        },
                    ))),
                    Err(error) if is_optional => {
                        console::warn(&format!(
                            "Skipping optional dependency {}@{}: {}",
                            name, range, error
                        ));
                        Ok(None)
                    }
                    Err(error) => Err(error),
                }
            });
        }

        let mut root_dependencies = BTreeMap::new();
        for result in join_all(tasks).await {
            if let Some((name, dependency)) = result? {
                root_dependencies.insert(name, dependency);
            }
        }

        Ok(ResolutionRoot {
            dependencies: root_dependencies,
        })
    }
}

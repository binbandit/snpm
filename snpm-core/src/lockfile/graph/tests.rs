use super::to_graph;
use crate::lockfile::{LockPackage, LockRoot, LockRootDependency, Lockfile};
use crate::project::BinField;
use crate::resolve::PackageId;

use std::collections::BTreeMap;

#[test]
fn to_graph_skips_unresolved_optional_roots() {
    let lockfile = Lockfile {
        version: 1,
        root: LockRoot {
            dependencies: BTreeMap::from([
                (
                    "required".to_string(),
                    LockRootDependency {
                        requested: "^1.0.0".to_string(),
                        package: None,
                        version: Some("1.2.3".to_string()),
                        optional: false,
                    },
                ),
                (
                    "optional".to_string(),
                    LockRootDependency {
                        requested: "^2.0.0".to_string(),
                        package: None,
                        version: None,
                        optional: true,
                    },
                ),
            ]),
        },
        packages: BTreeMap::new(),
    };

    let graph = to_graph(&lockfile);

    assert!(graph.root.dependencies.contains_key("required"));
    assert!(!graph.root.dependencies.contains_key("optional"));
}

#[test]
fn to_graph_reconstructs_dependencies() {
    let lockfile = Lockfile {
        version: 1,
        root: LockRoot {
            dependencies: BTreeMap::from([(
                "express".to_string(),
                LockRootDependency {
                    requested: "^4.0.0".to_string(),
                    package: None,
                    version: Some("4.18.2".to_string()),
                    optional: false,
                },
            )]),
        },
        packages: BTreeMap::from([
            (
                "express@4.18.2".to_string(),
                LockPackage {
                    name: "express".to_string(),
                    version: "4.18.2".to_string(),
                    tarball: "https://registry.npmjs.org/express/-/express-4.18.2.tgz".to_string(),
                    integrity: Some("sha512-abc123".to_string()),
                    dependencies: BTreeMap::from([(
                        "body-parser".to_string(),
                        "body-parser@1.20.0".to_string(),
                    )]),
                    bundled_dependencies: None,
                    has_bin: true,
                    bin: Some(BinField::Single("cli.js".to_string())),
                },
            ),
            (
                "body-parser@1.20.0".to_string(),
                LockPackage {
                    name: "body-parser".to_string(),
                    version: "1.20.0".to_string(),
                    tarball: "https://registry.npmjs.org/body-parser/-/body-parser-1.20.0.tgz"
                        .to_string(),
                    integrity: None,
                    dependencies: BTreeMap::new(),
                    bundled_dependencies: None,
                    has_bin: false,
                    bin: None,
                },
            ),
        ]),
    };

    let graph = to_graph(&lockfile);

    assert!(graph.root.dependencies.contains_key("express"));
    assert_eq!(
        graph.root.dependencies["express"].resolved.version,
        "4.18.2"
    );

    let express_id = PackageId {
        name: "express".to_string(),
        version: "4.18.2".to_string(),
    };
    let express = graph.packages.get(&express_id).unwrap();
    assert!(express.has_bin);
    assert!(matches!(express.bin, Some(BinField::Single(ref script)) if script == "cli.js"));
    assert!(express.dependencies.contains_key("body-parser"));
    assert_eq!(express.dependencies["body-parser"].version, "1.20.0");
}

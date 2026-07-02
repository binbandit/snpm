//! The environment variables npm/pnpm/yarn expose to lifecycle and
//! `run` scripts.
//!
//! Many real-world scripts branch on these — husky and patch-package
//! read `INIT_CWD`, version-stamping and monorepo tooling read
//! `npm_package_*`, and pre/post hooks read `npm_lifecycle_event`. A
//! script that only ever saw `PATH` (snpm's previous behavior) would
//! silently misbehave. We set the widely-relied-on subset here so
//! packages that "just work" under npm also work under snpm.

use std::path::Path;
use std::process::Command;
use std::sync::OnceLock;

/// The directory the top-level snpm command was invoked from. Captured
/// once at process start, before any `current_dir` change, so nested
/// script spawns all report the same `INIT_CWD` npm guarantees.
static INIT_CWD: OnceLock<Option<std::path::PathBuf>> = OnceLock::new();

fn init_cwd() -> Option<&'static Path> {
    INIT_CWD
        .get_or_init(|| std::env::current_dir().ok())
        .as_deref()
}

/// Apply the npm-compatible script environment to `command`.
///
/// `root` is the directory the script runs in (used for
/// `npm_package_json`); `name`/`version` come from the running package's
/// manifest; `lifecycle_event` is the script name (e.g. `postinstall`,
/// or a `run` target).
pub fn apply(
    command: &mut Command,
    root: &Path,
    name: Option<&str>,
    version: Option<&str>,
    lifecycle_event: &str,
) {
    command.env("npm_lifecycle_event", lifecycle_event);
    command.env("npm_package_json", root.join("package.json"));

    if let Some(name) = name {
        command.env("npm_package_name", name);
    }
    if let Some(version) = version {
        command.env("npm_package_version", version);
    }

    // The path to the package manager itself — tools like node-gyp and
    // npm-run-all re-invoke it through this.
    if let Ok(exe) = std::env::current_exe() {
        command.env("npm_execpath", exe);
    }

    command.env(
        "npm_config_user_agent",
        concat!("snpm/", env!("CARGO_PKG_VERSION")),
    );

    if let Some(cwd) = init_cwd() {
        command.env("INIT_CWD", cwd);
    }
}

#[cfg(test)]
mod tests {
    use super::apply;
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::path::Path;
    use std::process::Command;

    fn envs(command: &Command) -> BTreeMap<String, String> {
        command
            .get_envs()
            .filter_map(|(key, value)| {
                Some((
                    key.to_string_lossy().into_owned(),
                    value?.to_string_lossy().into_owned(),
                ))
            })
            .collect()
    }

    #[test]
    fn sets_npm_lifecycle_and_package_vars() {
        let mut command = Command::new("true");
        apply(
            &mut command,
            Path::new("/proj"),
            Some("my-pkg"),
            Some("1.2.3"),
            "postinstall",
        );
        let env = envs(&command);
        assert_eq!(
            env.get("npm_lifecycle_event").map(String::as_str),
            Some("postinstall")
        );
        assert_eq!(
            env.get("npm_package_name").map(String::as_str),
            Some("my-pkg")
        );
        assert_eq!(
            env.get("npm_package_version").map(String::as_str),
            Some("1.2.3")
        );
        assert_eq!(
            env.get("npm_package_json").map(OsString::from),
            Some(OsString::from("/proj/package.json"))
        );
        assert!(env.contains_key("npm_config_user_agent"));
        assert!(env.contains_key("INIT_CWD"));
    }

    #[test]
    fn omits_name_and_version_when_absent() {
        let mut command = Command::new("true");
        apply(&mut command, Path::new("/proj"), None, None, "build");
        let env = envs(&command);
        assert_eq!(
            env.get("npm_lifecycle_event").map(String::as_str),
            Some("build")
        );
        assert!(!env.contains_key("npm_package_name"));
        assert!(!env.contains_key("npm_package_version"));
    }
}

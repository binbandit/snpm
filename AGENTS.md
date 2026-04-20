# AGENTS.md

This is the primary context file for autonomous coding agents working in the `snpm` repository.

## Project Snapshot

`snpm` is a Rust-native, pnpm-style package manager with workspace-aware dependency install, lockfile-driven caching, strict script policy controls, and command parity for common package-manager workflows.

The implementation is split by role:

- `snpm-cli`: command-line parsing (Clap), command dispatch, and console UX.
- `snpm-core`: package/config resolution, store and linker internals, lockfile/integrity handling, registry/auth, workspace logic, lifecycle policies, and all install/build operation surfaces.
- `snpm-semver`: npm-compatible semantic version parsing and constraint resolution.
- `snpm-switch`: helper binary for loading the correct `snpm` runtime when a project declares a pinned `packageManager` version.

## Workspace and Build

- Workspace: `snpm-core`, `snpm-cli`, `snpm-semver`, `snpm-switch`.
- `cargo build --workspace`
- `cargo test --workspace` (or `cargo test --package <name>`)
- `cargo fmt --all`
- `cargo clippy --all-targets --all-features`
- `cargo install --path snpm-cli --force`

## CLI Command Surface (`snpm`)

Top-level commands in `snpm-cli/src/cli.rs`:

- `install`
- `add` (`--global`, `-g` / workspace filtering supported)
- `remove` (`--global`, `-g` / workspace filtering supported)
- `run` (lazy install when stale)
- `exec` (lazy install when stale)
- `init`
- `dlx` (`--offline`, `--prefer-offline`)
- `upgrade`
- `outdated`
- `licenses`
- `link`
- `list`
- `login`
- `logout`
- `config`
- `pack`
- `publish`
- `rebuild`
- `patch` (`edit`, `commit`, `remove`, `list`)
- `clean`
- `audit` (`--fix` path available)
- `why`
- `store` (`status`, `prune`, `path`)
- `unlink`
- Hidden internal: `completions` (for shell completion generation)

CLI compatibility behavior:

- `snpm run`/`snpx`/`pnpx` mapping is handled through argv rewriting in `snpm-cli/src/main.rs`.
- Unknown top-level subcommands are interpreted as package scripts, effectively enabling `snpm <script> ...` fallback for package.json scripts.

## `snpm-core` Module and API Layout

Public module exports in `snpm-core/src/lib.rs`:

- `config`, `cache`, `registry`, `protocols`
- `project`, `workspace`
- `resolve`, `lockfile`, `store`, `linker`
- `operations` (install, run, dlx, add/remove/rebuild logic, auth, audit, patch, publish, etc.)
- `lifecycle`, `console`, `http`
- Re-exported types: `SnpmConfig`, `HoistingMode`, `LinkBackend`, `OfflineMode`, `SnpmError`, `Project`, `Workspace`

## Runtime / Data Flow

1. `snpm` parses CLI args, applies multicall aliases if needed, and dispatches to a command handler.
2. `SnpmConfig::from_env()` builds config from:
   - `SNPM_HOME` and platform dirs
   - environment variables
   - `.snpmrc`, `.npmrc`, `.pnpmrc` from home and ancestor directories.
3. Command handlers resolve project/workspace from CWD, including workspace discovery (`snpm-workspace.yaml`, `pnpm-workspace.yaml`, and `package.json.workspaces`).
4. Install-like commands build `InstallOptions` and route into `snpm-core::operations`.
5. Resolver computes/loads dependency graph and lockfile inputs.
6. Store layer ensures tarballs are materialized into cache and extracted with `.snpm_complete` markers.
7. Linker builds `node_modules/.snpm` virtual store and root links (`link_dir` / symlink/copy/back-end behavior).
8. Integrity markers are computed and written; hot-path checks use them to short-circuit install work.
9. Lifecycle scripts run according to policy (`SNPM_ALLOW_SCRIPTS` and workspace overrides).

## Workspace, Catalogs, Overrides, and Filtering

- Workspace discovery supports `snpm-workspace.yaml`, `pnpm-workspace.yaml`, and `package.json` workspaces.
- Additional catalogs and overrides:
  - `snpm-catalog.yaml`
  - `snpm-overrides.yaml`
- Workspace config supports:
  - `onlyBuiltDependencies`
  - `ignoredBuiltDependencies`
  - optional workspace-level `hoisting`
- Many commands support workspace fan-out and selector filtering:
  - `--recursive`
  - `--filter`
  - `--filter-prod`

## Lockfile and Integrity

- Lockfile path: `snpm-lock.yaml`.
- Schema version currently supported: `1`.
- Root entries store `requested` plus `version`/`optional` metadata.
- Package entries include name/version/tarball/integrity/dependencies/bundledDependencies/hasBin.
- Lockfile writes occur for install-plan paths where `include_dev = true`; production-only paths skip dev resolution and avoid writing dev-inclusive lockfile updates.
- Integrity file: `node_modules/.snpm-integrity` at project root and per-project in workspace installs.
- Hot install path validates cached install state against lockfile-derived integrity.

## Data Paths and Layout

If `SNPM_HOME` is set:

- cache: `<SNPM_HOME>/cache`
- data: `<SNPM_HOME>/data`

Otherwise:

- cache/data: platform `directories` equivalents (for macOS data path is `~/Library/Application Support/io.snpm.snpm` via `directories`).

Derived data directories from `SnpmConfig`:

- packages: `<data_dir>/packages`
- metadata: `<data_dir>/metadata`
- global installs: `<data_dir>/global`
- global bins: `<data_dir>/bin`
- package cache layout: `<packages>/<name_with_/__sanitized>/<version>`

Global install storage is managed via `snpm add -g` and `snpm remove -g` and symlinked into `<data_dir>/bin`.

## Config and Auth Sources

Environment keys currently in use:

- `SNPM_HOME`
- `SNPM_ALLOW_SCRIPTS`
- `SNPM_MIN_PACKAGE_AGE_DAYS`
- `SNPM_MIN_PACKAGE_CACHE_AGE_DAYS`
- `SNPM_HOIST`
- `SNPM_LINK_BACKEND`
- `SNPM_STRICT_PEERS`
- `SNPM_FROZEN_LOCKFILE`
- `SNPM_REGISTRY_CONCURRENCY`
- `SNPM_VERBOSE`
- `SNPM_LOG_FILE`
- `NPM_CONFIG_REGISTRY` / `npm_config_registry`
- `NPM_CONFIG__AUTH` / `npm_config__auth`
- `NODE_AUTH_TOKEN` / `NPM_TOKEN` / `SNPM_AUTH_TOKEN`
- `NPM_CONFIG_ALWAYS_AUTH` / `npm_config_always_auth` / `SNPM_ALWAYS_AUTH`

RC file parsing (`.snpmrc`, `.npmrc`, `.pnpmrc`) is currently home-first then ancestor-ascending, with later files overriding earlier values.

Auth persistence is written to:
- `~/.snpmrc` (or fallback to local `.snpmrc` if home dirs unavailable).

## Registry / Protocol Support

- `npm` (default registry and scoped registries, npm-style metadata flow)
- `jsr` (adapted through npm-compatible metadata path)
- `file`
- `git`

## Operation Highlights

- Script policy is conservative by default:
  - dependency lifecycle scripts run only when allowlisted by env/workspace policy
  - root project scripts still run with `--run`/install script stages in the expected lifecycle sequence (`preinstall`, `install`, `postinstall`, `prepare`).
- `run`/`exec` can skip install with `--skip-install`.
- `link` supports local/project linking and global symlink flows.
- `store status/path/prune` expose cache health and cleanup hooks.
- `snpm-switch` is a separate binary intended for pin-aware launcher behavior and is not the primary CLI path for package operations.

## Code Style / Operating Notes

- Keep implementations straightforward and readable.
- Avoid unsafe and macro-heavy abstractions.
- Prefer explicit names and obvious control flow for maintainability.
- Add comments only where non-obvious behavior needs context.

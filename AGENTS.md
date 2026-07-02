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
- `audit` (`--fix` reports advisories with patched versions available and how to apply them; it does not mutate the manifest)
- `why`
- `store` (`status`, `prune`, `path`)
- `unlink`
- `node` (`install`, `uninstall`, `use`, `list`, `ls-remote`, `current`, `which`, `alias`, `unalias`, `default`, `exec`, `run`, `env`) — nvm-style Node version manager
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
- `node` (Node.js download/install, alias and current-version pointers, project pin discovery, shell init, and the bin-dir helper used by run/exec/lifecycle to auto-switch)
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
8. Root linking converges `node_modules` to the resolved graph: stale root links that point into a snpm virtual store (removed/dev-omitted packages) are pruned along with their dangling `.bin` launchers; entries from `snpm link`, `file:`/`link:` deps, and workspace cross-links are never touched.
9. Integrity markers are computed and written; hot-path checks use them to short-circuit install work.
10. Lifecycle scripts run according to policy (`SNPM_ALLOW_SCRIPTS` and workspace overrides).

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
- Root entries store `requested` plus `version`/`optional` metadata, and `package` when the entry is an npm alias (edge name differs from the resolved package).
- Package entries include name/version/tarball/integrity/dependencies/peerDependencies/bundledDependencies/hasBin. Packages are keyed by their *resolved* identity (`real-name@version`); alias names live only on edges.
- Peer ranges are persisted so `SNPM_STRICT_PEERS` validation also works for graphs rebuilt from the lockfile, not just cold resolves.
- A binary sidecar (`snpm-lock.bin`, format v3) is written beside the YAML for fast loads; it embeds a SHA-256 of the YAML and falls back to YAML parsing on any mismatch.
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
- Node.js versions: `<data_dir>/node/versions/<vX.Y.Z>/`
- Node aliases: `<data_dir>/node/aliases/<name>` (plain-text alias → version)
- Node current pointer: `<data_dir>/node/current` (plain-text active version)
- Node release-index cache: `<cache_dir>/node/index.json` (6h TTL)

Packages are materialized project-locally (in `<root>/.snpm`) instead of in the shared global virtual store when they are not global-store-safe: patched packages, packages whose lifecycle scripts are allowlisted, `file:`-sourced packages, and **any package with a required peer dependency**. A shared global-store entry has nothing above it in the module tree, so Node's upward resolution walk for a peer (`react-dom` → `react`) would dead-end in `<data_dir>/virtual-store`; keeping peer-having packages project-local preserves the peer via the project's root `node_modules`. This locality decision propagates to dependents (a package depending on a project-local package is itself project-local) and is folded into the install layout hash, so a node_modules produced by an older build self-heals on the next `snpm install`.

Global installs are a managed snpm project at `<data_dir>/global`: its `package.json` dependencies are the globally installed packages, and `snpm add -g` / `snpm remove -g` run the standard install pipeline against it (full dependency tree, lockfile, virtual store, hot path). Each installed package's bins are linked flat into `<data_dir>/bin` (the directory users put on PATH); removal prunes launchers whose targets vanished. `snpm list -g` reads the managed manifest.

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
- `SNPM_NODE_AUTO` (set to `0`/`false`/`off` to disable Node auto-switch in `run`/`exec`/lifecycle scripts)
- `SNPM_NODE_AUTO_INSTALL` (set to `0` to fail rather than download a missing pinned Node version)
- `SNPM_NODE_BIN_OVERRIDE` (explicit Node `bin/` dir to prepend; used by `snpm node run` and exposed for callers)
- `SNPM_NODE_DISTRO_URL` (override the default `https://nodejs.org/dist` source)
- `SNPM_NODE_SKIP_CHECKSUM` (skip SHASUMS256.txt verification when installing Node — for diagnostics only)
- `SNPM_REMOTE_CACHE_URL` (base URL of a remote side-effects cache, e.g. `https://cache.example.com/snpm`; GET on restore + PUT on save)
- `SNPM_REMOTE_CACHE_TOKEN` (bearer token sent as `Authorization: Bearer <token>` on remote-cache requests; optional)
- `SNPM_REMOTE_CACHE_READ_ONLY` (`1`/`true` to read from the remote cache but never PUT — useful for CI consumers that should not pollute a shared cache)
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
- `snpm node` manages Node.js versions (downloads from `nodejs.org/dist`, verifies SHASUMS256.txt) and pins via `.node-version`, `.nvmrc`, or `engines.node` (exact versions, partials like `20`, and ranges all match installed versions). `snpm run`/`snpm exec` prepend the matching Node `bin/` directory to `PATH` automatically and download a missing pinned version on demand; `SNPM_NODE_AUTO_INSTALL=0` makes an unsatisfied pin a hard error instead. Lifecycle scripts use the offline pin lookup only. The release-index cache is served stale when the network is down. `snpm node env --shell <sh>` emits a hook for interactive `cd` auto-switching.

## Code Style / Operating Notes

- Keep implementations straightforward and readable.
- Avoid unsafe and macro-heavy abstractions.
- Prefer explicit names and obvious control flow for maintainability.
- Add comments only where non-obvious behavior needs context.

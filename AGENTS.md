# AGENTS.md

This is the primary context file for autonomous coding agents working in the `snpm` repository.

## Project Snapshot

`snpm` is a pnpm-compatible package manager written in Rust. The current implementation focuses on fast installs, deterministic lockfile behavior, workspace support, patching, and safety defaults (script blocking and package-age controls).

## Workspace Crates

- `snpm-cli`: user-facing CLI and subcommand routing.
- `snpm-core`: install/resolve/link/store/config/auth/workspace/patch logic.
- `snpm-semver`: npm-style semver parser/range matching.
- `snpm-switch`: downloads/caches and runs the `snpm` version pinned by `packageManager`.

## Build and Development

```bash
# Build
cargo build

# Run tests
cargo test --workspace

# Run a single test
cargo test --workspace test_name

# Format (required before commit)
cargo fmt --all

# Lint
cargo clippy --all-targets --all-features

# Install locally
cargo install --path snpm-cli --force

# Alternative task runner
just install
```

## CLI Command Surface (`snpm`)

Implemented top-level commands in `snpm-cli/src/cli.rs`:

- `install`
- `add` (`--global` supported)
- `remove` (`--global` supported)
- `run`
- `exec`
- `init`
- `dlx`
- `upgrade`
- `outdated`
- `list`
- `login`
- `logout`
- `config`
- `patch` (`edit|commit|remove|list`)
- `clean`
- `audit`

## snpm-core Module Map

Key modules exported by `snpm-core/src/lib.rs`:

- `config`, `cache`, `registry`, `protocols`
- `project`, `workspace`
- `resolve`, `lockfile`, `store`, `linker`
- `operations` (install/add/remove/upgrade/outdated/run/exec/dlx/global/auth/patch/audit/clean)
- `lifecycle` (install script handling)

## Runtime Data Flow

1. CLI parses args and builds `SnpmConfig`.
2. Operation discovers project/workspace and reads manifests.
3. Resolver creates/updates dependency graph (from lockfile or registry metadata).
4. Store ensures package data in global cache (`.snpm_complete` marker).
5. Linker populates `node_modules/.snpm` virtual store and root links/binaries.
6. Install writes `node_modules/.snpm-integrity` for fast hot-path checks.

## Lockfile and Integrity

- Lockfile: `snpm-lock.yaml`, schema version `1`.
- Root entries store `requested` range and resolved `version`.
- Package entries store `name`, `version`, `tarball`, optional `integrity`, dependency map, optional `bundledDependencies`, and `hasBin`.
- Lockfile is written only when `include_dev = true` (for example normal install/add; not production-only install).
- Integrity file is `node_modules/.snpm-integrity` and stores a lockfile-derived hash.

## Store, Cache, and Global Paths

`SnpmConfig` resolves `cache_dir` and `data_dir` using platform dirs, or `SNPM_HOME`:

- If `SNPM_HOME` is set:
  - cache: `$SNPM_HOME/cache`
  - data: `$SNPM_HOME/data`
- Otherwise (via `directories::ProjectDirs("io", "snpm", "snpm")`):
  - macOS data: `~/Library/Application Support/io.snpm.snpm`
  - Linux data: `~/.local/share/snpm`
  - Windows data: `%LOCALAPPDATA%/snpm/snpm/data`

Derived directories:

- packages: `<data_dir>/packages`
- metadata: `<data_dir>/metadata`
- global installs: `<data_dir>/global`
- global bins: `<data_dir>/bin`

Package store layout:

- `<packages_dir>/<name_with_slash_replaced_by_underscore>/<version>/...`
- `.snpm_complete` marks successful extraction.

## Workspace and Catalog Conventions

Workspace discovery supports:

- `snpm-workspace.yaml`
- `pnpm-workspace.yaml`
- `package.json` `workspaces` field

Catalog and overrides support:

- `snpm-catalog.yaml`
- `snpm-overrides.yaml`

Workspace script policy fields in YAML:

- `onlyBuiltDependencies`
- `ignoredBuiltDependencies`

## Config and Auth Sources

Registry and install behavior is sourced from env and rc files.

Read rc files from home and current-directory ancestry:

- `.snpmrc`
- `.npmrc`
- `.pnpmrc`

Important env vars include:

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
- npm-compatible registry/auth vars such as `NPM_CONFIG_REGISTRY`, `NODE_AUTH_TOKEN`, `NPM_TOKEN`

`login`/`logout` persist auth to `~/.snpmrc`.

## Protocol Support

Resolver/registry path supports at least:

- `npm`
- `jsr` (resolved through npm-compatible metadata flow)
- `file`
- `git`

## Operational Behaviors Worth Knowing

- Install scenarios: `Hot`, `WarmLinkOnly`, `WarmPartialCache`, `Cold`.
- `install --production` skips dev-only dependencies and avoids lockfile mutation.
- `run`/`exec` perform lazy install unless `--skip-install` is used.
- `dlx` supports `--offline` and `--prefer-offline`.
- Link backend supports `auto`, `hardlink`, `symlink`, `copy`.
- Hoisting modes are `none`, `single-version`, and `all`.

## Code Style Rules

Follow the repository's "no cleverness" rule:

- Prefer straightforward, readable Rust over abstraction-heavy code.
- Avoid `unsafe` and avoid complex macro-based magic.
- Keep naming and control flow obvious to a mid-level Rust developer.
- Add comments only for genuinely non-obvious logic.

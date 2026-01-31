# AGENTS.md

This file serves as the **primary context and instruction manual** for autonomous agents working on the `snpm` (Speedy Node Package Manager) repository.

## 1. Vision

`snpm` is a **drop‑in, modern replacement for npm/yarn/pnpm** that:

-   Feels familiar enough that developers can switch in minutes.
-   Is **fast by default** via a global cache, parallelism, and smart reuse.
-   Is **deterministic and reliable** via a simple, readable lockfile.
-   Has a **clean, beautiful implementation** in Rust with a strict **"no cleverness"** rule.

**Goal:** We aren't trying to invent bizarre new workflows. We want the day‑to‑day "install deps, run scripts, update stuff, work in monorepos" story to be faster, simpler, and easier to maintain.

## 2. Design Principles

### Product Principles

1.  **Familiar, not surprising**: Commands (`install`, `add`, `remove`, `run`) should match developer muscle memory.
2.  **Fast for real projects**: Use global store, parallel network/disk options, and avoid unnecessary re-resolution.
3.  **Deterministic installs**: Lockfile (`snpm-lock.yaml`) must be checked into git and guarantee repeatable installs.
4.  **Monorepo‑ready**: Workspaces are first-class, not bolted on.

### Implementation Principles (Rust)

1.  **"No Cleverness" Rule**: Every line must have a clear purpose. No premature abstractions or generic frameworks.
2.  **Self‑documenting code**: Avoid comments unless an algorithm is genuinely hard to follow. Names and structure should make intent obvious.
3.  **Readable by mid‑level Rust devs**: No macros beyond standard derive. No `unsafe`. Avoid complex type magic.
4.  **Stupid simple, not stupid**: Prefer obviously correct linear concurrency over "optimal" but complex logic.

## 3. High‑Level Architecture

### Crate Layout

-   **`snpm-cli`**: CLI binary, argument parsing, wiring into core. Depends on `snpm-core`.
-   **`snpm-core`**: All actual logic (Config, Registry, Resolution, Store, Linking, Operations).
-   **`snpm-semver`**: Custom semver range parsing for npm-style ranges.
-   **`snpm-switch`**: Version manager for project-specific snpm versions.

### Core Modules (`snpm-core`)

-   **`config`**: `SnpmConfig`, directory handling, rc file parsing.
-   **`project`**: `Manifest` (package.json) parsing, project discovery.
-   **`workspace`**: Multi-project/monorepo support, catalog resolution.
-   **`registry`**: NPM registry API interaction (supports npm, jsr, custom registries).
-   **`resolve`**: Dependency resolution (Semver parsing, graph building, peer deps).
-   **`store`**: Global package cache management. Downloads and unpacks tarballs.
-   **`lockfile`**: `snpm-lock.yaml` reading/writing.
-   **`linker`**: Builds `node_modules` via virtual store (`.snpm/`). Supports hoisting modes.
-   **`operations`**: High-level commands (`install`, `add`, `remove`, `run`, `dlx`, `init`, `patch`, `clean`, etc.).
-   **`protocols`**: Protocol handlers (npm, file, git, jsr).
-   **`lifecycle`**: Install script execution with security controls.
-   **`error`**: Centralized `SnpmError` enum.

## 4. Core Flows

### 4.1 `snpm install`

1.  **Discover**: Find project or workspace.
2.  **Determine Roots**: Merge `dependencies`, `devDependencies` (if applicable), and CLI args.
3.  **Resolve**:
    -   If `include_dev == true` and lockfile matches manifest: Rebuild graph from lockfile (fast).
    -   Else: Resolve from registry and write lockfile (if `include_dev == true`).
4.  **Materialize Store**: Download and unpack missing packages to global store in parallel.
5.  **Write Manifest**: Update `package.json` if packages were requested via CLI.
6.  **Link**: Clear `node_modules` and reconstruct it using symlinks/copies from the store. Build `.bin` executables.

### 4.2 `snpm add`

Same as `install <pkg>...` but always includes dev deps (`include_dev = true`). Saves to `dependencies` (default) or `devDependencies` (if `-D` flag used).

### 4.3 `snpm remove`

Removes names from manifest, writes `package.json`, then runs `install` to recompute graph and clean `node_modules`.

### 4.4 `snpm run`

Looks up script in `manifest.scripts`, runs via `sh -c` (Unix) or `cmd /C` (Windows) with `node_modules/.bin` prepended to `PATH`.

## 5. Data Formats

### Manifest (`package.json`)

Standard `package.json` structure.
-   `dependencies` / `devDependencies`: Semver ranges.
-   `scripts`: Command strings.

### Lockfile (`snpm-lock.yaml`)

-   **Format**: `version: 1`
-   **Root Deps**: `name -> { requested, version }`
-   **Packages**: Flat list of resolved packages (`name@version`) containing tarball URL, integrity, and dependencies.

## 6. Global Store Design

-   **Location**: `SnpmConfig::packages_dir()` (platform-specific cache dir).
-   **Structure**: `<packages_dir>/<sanitizedName>/<version>/...`
-   **Marker**: Uses `.snpm_complete` file to verify package integrity.
-   **Concurrency**: Populated in parallel using `join_all`.

## 7. Roadmap & Agent Tasks

Agents should refer to this roadmap when selecting tasks or prioritizing work.

### Completed
-   [x] Full CLI (`install`, `add`, `remove`, `run`, `init`, `upgrade`, `outdated`, `dlx`, `exec`, `list`, `patch`, `clean`, `config`, `login`, `logout`)
-   [x] Global store with parallel downloads
-   [x] Lockfile read/write with `--frozen-lockfile` support
-   [x] Advanced semver support (including `||` ranges)
-   [x] Virtual store layout (`.snpm/`) with configurable hoisting
-   [x] First-class workspaces with catalog protocol
-   [x] Multiple link backends (auto, hardlink, symlink, copy)
-   [x] Protocol support (npm, file, git, jsr, workspace, catalog)
-   [x] Install script security (blocked by default)
-   [x] Minimum version age protection
-   [x] Global package installation (`-g` flag)
-   [x] Package patching (`snpm patch`)

### Next Steps (Priorities)
1.  **Publishing**: `snpm publish` with workspace support.
2.  **Dependency Analysis**: `snpm why <package>` to explain dependency paths.
3.  **Performance**: Smarter resolution graph reuse within workspaces.

## 8. Workflow Reminders

-   **Build**: `cargo build`
-   **Test**: `cargo test`
-   **Format**: `cargo fmt` (Mandatory)
-   **Lint**: `cargo clippy`

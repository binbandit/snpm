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

### Core Modules (`snpm-core`)

-   **`config`**: `SnpmConfig`, directory handling.
-   **`project`**: `Manifest` (package.json) parsing, project discovery.
-   **`registry`**: NPM registry API interaction (`https://registry.npmjs.org`).
-   **`resolve`**: Dependency resolution (Semver parsing, deep recursion, graph building).
-   **`store`**: Global package cache management. Downloads and unpacks tarballs.
-   **`lockfile`**: `snpm-lock.yaml` reading/writing.
-   **`linker`**: Builds `node_modules` from the resolution graph and global store. Clears `node_modules` on each run.
-   **`operations`**: High-level commands (`install`, `add`, `remove`, `run`).
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

### Completed / In-Progress
-   [x] Basic CLI (`install`, `add`, `remove`, `run`)
-   [x] Global store & Parallel downloads
-   [x] Lockfile read/write
-   [x] Semver support
-   [x] `node_modules` linking

### Next Steps (Priorities)
1.  **Refine Store + Linking**: Reduce disk usage (symlinks vs copies).
2.  **First-class Workspaces**: Resolve local packages before registry. Single lockfile per workspace.
3.  **Lockfile Modes**: `-frozen-lockfile` (CI support).
4.  **Quality of Life**: `snpm init`, `snpm outdated`.

## 8. Workflow Reminders

-   **Build**: `cargo build`
-   **Test**: `cargo test`
-   **Format**: `cargo fmt` (Mandatory)
-   **Lint**: `cargo clippy`

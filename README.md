<div align="center">
  <img src="docs/logo.png" alt="snpm logo" width="300" />
  <p><strong>Speedy Node Package Manager</strong></p>
</div>

> **Status:** Production-ready. Fast, secure, and feature-complete for most workflows.

`snpm` is a drop-in replacement for npm, yarn, and pnpm. It is written in Rust with a strict "no cleverness" rule.

We aren't trying to invent a new workflow. We just want the existing one—installing deps, working in monorepos, running scripts—to be faster, simpler, and easier to maintain.

## The Vision

We want a tool that:
- **Feels Familiar**: `snpm install`, `snpm add`, `snpm run`. No new muscle memory required.
- **Is Fast by Default**: Global caching, parallel downloads, and smart reuse.
- **Is Deterministic**: A simple, readable lockfile (`snpm-lock.yaml`) that guarantees the same install everywhere.
- **Is Secure**: Install scripts blocked by default, minimum package age protection against supply chain attacks.
- **Is Readable**: The codebase is designed to be understood by mid-level Rust devs. No premature abstractions or complex type magic.

## Features

- **Full CLI**: `install`, `add`, `remove`, `run`, `init`, `upgrade`, `outdated`, `dlx`, `exec`, `list`, `patch`, `clean`, `config`, `login`, `logout`
- **Global Store**: Download packages once to a global cache and reuse them across projects.
- **Parallelism**: Network and disk operations happen in parallel where safe.
- **Workspaces**: First-class monorepo support with `snpm-workspace.yaml` (or `pnpm-workspace.yaml`).
- **Catalog Protocol**: Define versions in `snpm-catalog.yaml` and reference them across your workspace. No more version drift.
- **Security Features**:
  - Install scripts blocked by default (explicit whitelist required)
  - Minimum package age protection (`SNPM_MIN_PACKAGE_AGE_DAYS`)
  - Frozen lockfile support for CI (`--frozen-lockfile`)
- **Flexible Linking**: Virtual store layout with configurable hoisting (none, single-version, all) and link backends (auto, hardlink, symlink, copy).
- **Multiple Protocols**: npm, file, git, jsr, workspace, catalog.
- **Package Patching**: `snpm patch` to modify installed packages and auto-apply patches on install.

## Installation

```bash
# Install via npm
npm install -g snpm

# Or build from source
cargo install --path snpm-cli
```

## Usage

```bash
# Install dependencies
snpm install

# Add packages
snpm add react
snpm add -D typescript

# Run scripts
snpm run build

# Execute binaries
snpm exec tsc --version

# Upgrade dependencies
snpm upgrade

# Check for outdated packages
snpm outdated
```

## CI/CD

```bash
# Use frozen lockfile (fail if out of sync)
snpm install --frozen-lockfile

# Production install (skip devDependencies)
snpm install --production --frozen-lockfile

# With security settings
SNPM_MIN_PACKAGE_AGE_DAYS=7 snpm install --frozen-lockfile
```

## Documentation

Visit [snpm.io](https://snpm.io) for full documentation.

## License

MIT OR Apache-2.0

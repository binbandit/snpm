<div align="center">
  <img src="docs/logo.png" alt="snpm logo" width="300" />
  <p><strong>Speedy Node Package Manager</strong></p>
  <p>
    <a href="https://www.npmjs.com/package/snpm"><img src="https://img.shields.io/npm/v/snpm?style=flat-square&color=blue" alt="npm version"></a>
    <a href="https://github.com/binbandit/snpm/releases"><img src="https://img.shields.io/github/v/release/binbandit/snpm?style=flat-square&color=green" alt="GitHub release"></a>
    <a href="https://github.com/binbandit/snpm/blob/main/LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=flat-square" alt="License"></a>
  </p>
</div>

A fast, secure, drop-in replacement for npm, yarn, and pnpm — written in Rust.

## Why snpm?

| | snpm | npm | yarn | pnpm |
|---|:---:|:---:|:---:|:---:|
| Global package cache | ✅ | ❌ | ✅ | ✅ |
| Parallel downloads | ✅ | Limited | ✅ | ✅ |
| Install script blocking | ✅ | ❌ | ❌ | ❌ |
| Minimum package age | ✅ | ❌ | ❌ | ❌ |
| Readable lockfile (YAML) | ✅ | ❌ | ✅ | ✅ |
| Version catalogs | ✅ | ❌ | ❌ | ✅ |
| Written in Rust | ✅ | ❌ | ❌ | ❌ |

**Key differentiators:**
- **Security-first**: Install scripts blocked by default, minimum package age protection against supply chain attacks
- **Fast by default**: Global cache, parallel operations, smart lockfile reuse
- **Familiar**: Same commands you already know — `install`, `add`, `run`

## Installation

```bash
# Via npm (recommended)
npm install -g snpm

# Via GitHub releases (macOS, Linux, Windows)
# Download from https://github.com/binbandit/snpm/releases

# From source
cargo install --path snpm-cli
```

## Quick Start

```bash
# Install dependencies
snpm install

# Add packages
snpm add react
snpm add -D typescript

# Run scripts
snpm run build

# That's it. Same workflow, faster and more secure.
```

## Features

### Commands
`install` · `add` · `remove` · `run` · `exec` · `dlx` · `init` · `upgrade` · `outdated` · `list` · `patch` · `clean` · `config` · `login` · `logout`

### Workspaces
First-class monorepo support with `snpm-workspace.yaml` (or `pnpm-workspace.yaml` for easy migration).

### Catalog Protocol  
Define versions once in `snpm-catalog.yaml`, reference with `"react": "catalog:"` — no more version drift.

### Security
- **Install scripts blocked by default** — whitelist trusted packages explicitly
- **Minimum package age** — ignore packages published in the last N days
- **Frozen lockfile** — fail CI if lockfile is out of sync

### Flexible Linking
Virtual store layout with configurable hoisting (`none`, `single-version`, `all`) and link backends (`auto`, `hardlink`, `symlink`, `copy`).

### Package Patching
`snpm patch edit lodash` → make changes → `snpm patch commit` — patches auto-apply on install.

## CI/CD

```bash
# Recommended CI setup
SNPM_MIN_PACKAGE_AGE_DAYS=7 snpm install --frozen-lockfile

# Production build
snpm install --production --frozen-lockfile
```

## Documentation

Full documentation at **[snpm.io](https://snpm.io)**

## Contributing

We welcome contributions! snpm is written in Rust with a strict "no cleverness" rule — the codebase is designed to be readable by mid-level Rust developers.

```bash
git clone https://github.com/binbandit/snpm.git
cd snpm
cargo build
cargo test
```

## License

MIT OR Apache-2.0

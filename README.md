<div align="center">
  <img src="docs/logo.png" alt="snpm logo" width="300" />
  
  <p>
    <strong>A fast, secure, drop-in replacement for npm</strong>
  </p>

  <p>
    <a href="https://www.npmjs.com/package/snpm"><img src="https://img.shields.io/npm/v/snpm?style=flat-square" alt="npm version"></a>
    <a href="https://github.com/binbandit/snpm/releases"><img src="https://img.shields.io/github/v/release/binbandit/snpm?style=flat-square" alt="GitHub release"></a>
    <a href="https://github.com/binbandit/snpm/blob/main/LICENSE-MIT"><img src="https://img.shields.io/badge/license-MIT%2FApache--2.0-blue?style=flat-square" alt="License"></a>
  </p>

  <p>
    <a href="https://snpm.io">Documentation</a> · <a href="https://github.com/binbandit/snpm/releases">Releases</a> · <a href="https://github.com/binbandit/snpm/issues">Issues</a>
  </p>
</div>

<br />

## Install

```bash
npm install -g snpm
```

Or download a binary from [GitHub Releases](https://github.com/binbandit/snpm/releases).

<br />

## Usage

```bash
snpm install          # Install dependencies
snpm add react        # Add a package
snpm add -D typescript # Add a dev dependency
snpm run build        # Run a script
```

Same commands you already know. No new workflow to learn.

<br />

## Why snpm?

|  | snpm | npm | yarn | pnpm |
|:--|:--:|:--:|:--:|:--:|
| Global cache | ✅ | ❌ | ✅ | ✅ |
| Parallel downloads | ✅ | ⚠️ | ✅ | ✅ |
| Install script blocking | ✅ | ❌ | ❌ | ❌ |
| Min package age | ✅ | ❌ | ❌ | ❌ |
| Version catalogs | ✅ | ❌ | ❌ | ✅ |
| Written in Rust | ✅ | ❌ | ❌ | ❌ |

<br />

## Features

**Security** — Install scripts are blocked by default. Set `SNPM_MIN_PACKAGE_AGE_DAYS=7` to ignore packages published in the last week.

**Workspaces** — First-class monorepo support. Drop in `snpm-workspace.yaml` or use your existing `pnpm-workspace.yaml`.

**Catalogs** — Define versions once in `snpm-catalog.yaml`, reference them with `"react": "catalog:"`. No more version drift.

**Patching** — Run `snpm patch edit lodash`, make your fix, then `snpm patch commit`. Patches auto-apply on install.

**Flexible** — Configurable hoisting modes and link backends. Use hardlinks, symlinks, or copies.

<br />

## CI/CD

```bash
snpm install --frozen-lockfile                    # Fail if lockfile is out of sync
snpm install --production --frozen-lockfile       # Skip devDependencies
SNPM_MIN_PACKAGE_AGE_DAYS=7 snpm install          # Ignore recently published packages
```

<br />

## Commands

```
install    Install dependencies from package.json
add        Add packages to dependencies
remove     Remove packages
run        Run a script from package.json
exec       Execute a command with node_modules/.bin in PATH
dlx        Download and run a package (like npx)
init       Create a new package.json
upgrade    Update packages to latest versions
outdated   Check for outdated packages
list       List installed packages
patch      Patch installed packages
clean      Clear the package cache
config     Show configuration
login      Authenticate with a registry
logout     Remove registry credentials
```

<br />

## Documentation

**[snpm.io](https://snpm.io)**

<br />

## Contributing

```bash
git clone https://github.com/binbandit/snpm.git
cd snpm
cargo build && cargo test
```

snpm follows a "no cleverness" rule — code should be readable by any mid-level Rust developer.

<br />

## License

MIT OR Apache-2.0

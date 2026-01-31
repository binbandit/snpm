<div align="center">
  <img src="docs/logo.png" alt="snpm logo" width="300" />
  
  <p>
    <strong>A pnpm-compatible package manager, rewritten in Rust.</strong>
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

> **Note:** snpm is under active development. Core features work well, but we're still building toward full pnpm compatibility. [Contributions are welcome!](#contributing)

<br />

## What is snpm?

snpm is a drop-in replacement for pnpm, built from the ground up in Rust. Our goal is to match pnpm's functionality while delivering better performance and a cleaner developer experience.

**Current status:**
- ✅ Core commands working (`install`, `add`, `remove`, `run`, `exec`, `dlx`)
- ✅ Workspaces and catalogs
- ✅ Lockfile compatibility
- ✅ Security features (install script blocking, min package age)
- 🚧 Publishing (`snpm publish`)
- 🚧 Full pnpm CLI parity

<br />

## Install

```bash
npm install -g snpm
```

Or download a binary from [GitHub Releases](https://github.com/binbandit/snpm/releases).

<br />

## Usage

```bash
snpm install           # Install dependencies
snpm add react         # Add a package
snpm add -D typescript # Add a dev dependency
snpm run build         # Run a script
```

If you're coming from pnpm, snpm reads your existing `pnpm-workspace.yaml` and `pnpm-lock.yaml`.

<br />

## Why snpm?

We love pnpm. We just think it can be faster and simpler.

|  | snpm | pnpm |
|:--|:--:|:--:|
| Written in Rust | ✅ | ❌ |
| Install script blocking | ✅ | ❌ |
| Min package age protection | ✅ | ❌ |
| Reads pnpm config | ✅ | ✅ |
| Version catalogs | ✅ | ✅ |
| Full CLI parity | 🚧 | ✅ |

<br />

## Features

**Security first** — Install scripts are blocked by default. Packages must be explicitly whitelisted. Set `SNPM_MIN_PACKAGE_AGE_DAYS=7` to ignore recently published packages.

**pnpm compatible** — Reads `pnpm-workspace.yaml`, `pnpm-lock.yaml`, and `.npmrc`. Migration is straightforward.

**Workspaces & catalogs** — First-class monorepo support with version catalogs to eliminate drift.

**Patching** — `snpm patch edit lodash` → make changes → `snpm patch commit`. Patches auto-apply on install.

<br />

## CI/CD

```bash
snpm install --frozen-lockfile                    # Fail if lockfile is out of sync
snpm install --production --frozen-lockfile       # Skip devDependencies
SNPM_MIN_PACKAGE_AGE_DAYS=7 snpm install          # Ignore recently published packages
```

<br />

## Documentation

**[snpm.io](https://snpm.io)**

<br />

## Contributing

**We need your help!** snpm is actively looking for contributors. Whether it's bug reports, feature requests, documentation improvements, or code contributions — all are welcome.

```bash
git clone https://github.com/binbandit/snpm.git
cd snpm
cargo build && cargo test
```

See **[CONTRIBUTING.md](CONTRIBUTING.md)** for our development philosophy, code guidelines, and areas where we need help.

<br />

## License

MIT OR Apache-2.0

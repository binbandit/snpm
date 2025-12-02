# snpm

**Speedy Node Package Manager** - A fast, Rust-based package manager that's a drop-in replacement for npm, yarn, and pnpm.

## Installation

### Via npm

```bash
npm install -g snpm
```

### Via direct download

Download the latest release for your platform from [GitHub Releases](https://github.com/binbandit/snpm/releases).

## Usage

Use `snpm` just like you would use `npm`:

```bash
# Install dependencies
snpm install

# Add a package
snpm add react

# Remove a package
snpm remove lodash

# Run a script
snpm run build
```

## Features

- **Fast by Default**: Global caching, parallel downloads, and smart reuse
- **Deterministic**: Simple, readable lockfile (`snpm-lock.yaml`)
- **Workspace Support**: First-class monorepo support
- **Catalog Protocol**: Define versions in one place across your workspace
- **Minimum Version Age**: Protect against zero-day malicious packages

## Documentation

For more information, visit the [snpm repository](https://github.com/binbandit/snpm).

## License

MIT

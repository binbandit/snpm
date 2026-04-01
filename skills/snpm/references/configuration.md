# snpm configuration reference

## Environment variables

### Directories

| Variable | Description |
|----------|-------------|
| `SNPM_HOME` | Base directory. Sets `cache=$SNPM_HOME/cache`, `data=$SNPM_HOME/data` |

Without `SNPM_HOME`, platform directories are used:
- macOS: `~/Library/Application Support/io.snpm.snpm`
- Linux: `~/.local/share/snpm`
- Windows: `%LOCALAPPDATA%/snpm/snpm/data`

Derived directories under data dir: `packages/`, `metadata/`, `global/`, `bin/`.

### Registry and auth

| Variable | Description |
|----------|-------------|
| `NPM_CONFIG_REGISTRY` | Default registry URL (default: `https://registry.npmjs.org/`) |
| `NODE_AUTH_TOKEN` | Bearer token for default registry (highest priority) |
| `NPM_TOKEN` | Bearer token (medium priority) |
| `SNPM_AUTH_TOKEN` | Bearer token (lowest priority) |
| `NPM_CONFIG__AUTH` | Base64-encoded credentials for Basic auth |
| `NPM_CONFIG_ALWAYS_AUTH` | Force auth on all requests |

### Install behavior

| Variable | Default | Values |
|----------|---------|--------|
| `SNPM_HOIST` | `single-version` | `none`, `single-version`, `all` |
| `SNPM_LINK_BACKEND` | `auto` | `auto`, `hardlink`, `symlink`, `copy` |
| `SNPM_STRICT_PEERS` | `false` | `true`/`1`/`yes`/`y`/`on` |
| `SNPM_FROZEN_LOCKFILE` | `false` | `true`/`1`/`yes`/`y`/`on` |
| `SNPM_REGISTRY_CONCURRENCY` | `64` | Any integer > 0 |

### Security

| Variable | Default | Description |
|----------|---------|-------------|
| `SNPM_ALLOW_SCRIPTS` | (empty) | Comma-separated packages allowed to run install scripts |
| `SNPM_MIN_PACKAGE_AGE_DAYS` | (none) | Reject packages published within N days |
| `SNPM_MIN_PACKAGE_CACHE_AGE_DAYS` | `7` | Re-fetch metadata older than N days |

Dependency lifecycle scripts are blocked by default. Root project and workspace-member lifecycle scripts still run during install.

### Logging

| Variable | Default | Description |
|----------|---------|-------------|
| `SNPM_VERBOSE` | `false` | Enable verbose output |
| `SNPM_LOG_FILE` | (none) | Write logs to file |

## RC files

Read in order (last wins):
1. `~/.snpmrc`, `~/.npmrc`, `~/.pnpmrc`
2. Project root and ancestor directories: `.snpmrc`, `.npmrc`, `.pnpmrc`

### Syntax

```ini
# Comments start with # or ;
registry=https://registry.npmjs.org/

# Scoped registries
@myorg:registry=https://npm.myorg.com/

# Auth tokens per host
//npm.myorg.com/:_authToken=${NPM_TOKEN}
//npm.myorg.com/:_auth=dXNlcjpwYXNz

# Default registry auth
_authToken=abc123

# Hoisting
snpm-hoist=single-version

# Force auth
always-auth=true
```

Variable expansion with `${VAR_NAME}` or `$VAR_NAME` is supported.

## Workspace configuration

### Workspace discovery

Searched in order (first found wins):
1. `snpm-workspace.yaml`
2. `pnpm-workspace.yaml`
3. `package.json` `workspaces` field

### snpm-workspace.yaml

```yaml
packages:
  - "packages/*"
  - "apps/*"

catalog:
  react: "^18.0.0"
  typescript: "^5.0.0"

catalogs:
  tooling:
    eslint: "^8.0.0"
    prettier: "^3.0.0"

onlyBuiltDependencies:
  - esbuild
  - node-gyp

ignoredBuiltDependencies:
  - prebuild-install

hoisting: "single-version"
```

### snpm-catalog.yaml

Standalone version catalog merged into workspace config (workspace takes precedence):

```yaml
catalog:
  axios: "^1.6.0"
catalogs:
  testing:
    jest: "^29.0.0"
```

### snpm-overrides.yaml

Override dependency resolutions across the project.

### package.json workspaces

```json
{ "workspaces": ["packages/*", "apps/*"] }
```

## Lockfile

File: `snpm-lock.yaml` (schema version 1).

```yaml
version: 1
root:
  dependencies:
    express:
      requested: "^4.18.0"
      version: "4.18.2"
      optional: false
packages:
  express@4.18.2:
    name: express
    version: 4.18.2
    tarball: "https://registry.npmjs.org/express/-/express-4.18.2.tgz"
    integrity: "sha512-..."
    dependencies:
      body-parser: "body-parser@1.20.1"
    hasBin: true
```

Only written when `include_dev = true` (not with `--production`).

## Integrity

`node_modules/.snpm-integrity` stores a lockfile-derived hash for fast hot-path install detection.

## Install scenarios

| Scenario | Condition | Speed |
|----------|-----------|-------|
| Hot | Everything cached, integrity matches | ~100ms |
| WarmLinkOnly | Lockfile valid, all packages cached | ~1-5s |
| WarmPartialCache | Lockfile valid, some packages missing | ~5-30s |
| Cold | No valid lockfile or forced | Minutes |

## Store layout

Package store: `<data_dir>/packages/<name_slug>/<version>/`
- Scoped names: `/` becomes `_` (e.g., `@babel/core` → `@babel_core`)
- `.snpm_complete` marker indicates successful extraction

## Hoisting modes

| Mode | Behavior |
|------|----------|
| `none` | Each package in isolated tree |
| `single-version` (default) | Hoists single-version packages to root |
| `all` | Aggressive hoisting to root |

## Link backends

| Backend | Notes |
|---------|-------|
| `auto` (default) | Hardlinks on Unix, copies on Windows |
| `hardlink` | Fast, space-efficient |
| `symlink` | Cross-filesystem compatible |
| `copy` | Slowest, most compatible |

## CI/CD

```bash
snpm install --frozen-lockfile
snpm install --production --frozen-lockfile
SNPM_MIN_PACKAGE_AGE_DAYS=7 snpm install --frozen-lockfile
SNPM_FROZEN_LOCKFILE=1 snpm install
snpm audit --format sarif > snpm-audit.sarif
```

Cache across CI runs: `$DATA_DIR/packages/` and `$DATA_DIR/metadata/`.

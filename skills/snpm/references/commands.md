# snpm command reference

## install

Install dependencies for a project or workspace.

```
snpm install [packages...] [flags]
```

| Flag | Description |
|------|-------------|
| `--production` | Skip devDependencies (no lockfile mutation) |
| `--frozen-lockfile`, `--immutable` | Fail if lockfile is missing or out of date |
| `-f`, `--force` | Ignore cached state, force full install |
| `-w`, `--workspace <NAME>` | Target specific workspace package |

## add

Add packages to dependencies.

```
snpm add <packages...> [flags]
```

| Flag | Description |
|------|-------------|
| `-D`, `--dev` | Save to devDependencies |
| `-g`, `--global` | Install globally |
| `-f`, `--force` | Ignore cached state |
| `-w`, `--workspace <NAME>` | Target specific workspace package |

## remove

Remove packages from dependencies.

```
snpm remove <packages...> [flags]
```

| Flag | Description |
|------|-------------|
| `-g`, `--global` | Remove globally installed package |

## run

Run a package.json script.

```
snpm run <script> [-- args...] [flags]
```

| Flag | Description |
|------|-------------|
| `-r`, `--recursive` | Run in all workspace projects |
| `--filter <PATTERN>` | Filter workspace projects by name (glob patterns like `app-*`) |
| `--skip-install` | Skip automatic install check |

Unknown subcommands are treated as script names: `snpm build` is equivalent to `snpm run build`.

## exec

Execute a command with node_modules/.bin in PATH.

```
snpm exec <command> [args...] [flags]
```

| Flag | Description |
|------|-------------|
| `-c`, `--shell-mode` | Run through shell (enables pipes, redirects) |
| `-r`, `--recursive` | Run in all workspace projects |
| `--filter <PATTERN>` | Filter workspace projects by name |
| `--skip-install` | Skip automatic install check |

## dlx

Download and run a package without installing.

```
snpm dlx <package> [args...] [flags]
```

| Flag | Description |
|------|-------------|
| `--offline` | Fail if not cached |
| `--prefer-offline` | Prefer cache, fetch only if missing |

## upgrade

Upgrade dependencies and refresh the lockfile.

```
snpm upgrade [packages...] [flags]
```

| Flag | Description |
|------|-------------|
| `--production` | Skip devDependencies |
| `-f`, `--force` | Ignore cached state |

Without package arguments, deletes the lockfile for a full re-resolve.

## outdated

Check for outdated dependencies.

```
snpm outdated [flags]
```

| Flag | Description |
|------|-------------|
| `--production` | Skip devDependencies |

## list

List installed packages.

```
snpm list [flags]
```

| Flag | Description |
|------|-------------|
| `-g`, `--global` | List globally installed packages |

## why

Explain why a dependency is installed.

```
snpm why <package> [flags]
```

| Flag | Description |
|------|-------------|
| `--depth <N>` | Maximum reverse dependency depth |
| `--json` | JSON output |

Supports glob patterns like `babel-*`.

## audit

Scan dependencies for security vulnerabilities.

```
snpm audit [packages...] [flags]
```

| Flag | Description |
|------|-------------|
| `--audit-level <LEVEL>` | Minimum severity: `critical`, `high`, `moderate`, `low`, `info` |
| `-P`, `--prod` | Only production dependencies |
| `-D`, `--dev` | Only devDependencies |
| `--format <FORMAT>` | Output: `table` (default), `json`, `sarif` |
| `--fix` | Auto-upgrade vulnerable packages |
| `--ignore-cve <CVE>` | Ignore specific CVE (repeatable) |
| `--ignore-ghsa <GHSA>` | Ignore specific GHSA (repeatable) |
| `--ignore-unfixable` | Skip vulnerabilities with no available fix |
| `--ignore-registry-errors` | Continue with exit code 0 on registry errors |

Exits with code 1 if vulnerabilities are found. Works with workspaces.

## patch

Patch packages to fix bugs or customize behavior.

### patch edit / patch start

```
snpm patch edit <package[@version]>
```

Copies the installed package to a temp directory for editing. Creates a `.snpm_patch_session` marker.

### patch commit

```
snpm patch commit <edit_dir>
```

Generates a unified diff from the edited directory. Stores the patch at `patches/<package>@<version>.patch` and updates `package.json` with the patch entry under `snpm.patched_dependencies`.

Scoped package names use `+` instead of `/` in the filename (e.g., `+babel+core@7.0.0.patch`).

### patch remove

```
snpm patch remove <package>
```

Deletes the patch file and removes the entry from package.json.

### patch list

```
snpm patch list
```

Lists all patches in the project.

### Patch manifest format

```json
{
  "snpm": {
    "patched_dependencies": {
      "lodash@4.17.21": "patches/lodash@4.17.21.patch"
    }
  }
}
```

Also compatible with `pnpm.patched_dependencies`.

## login

Authenticate with a registry.

```
snpm login [flags]
```

| Flag | Description |
|------|-------------|
| `--registry <URL>` | Registry URL |
| `--scope <SCOPE>` | Associate credentials with scope (e.g., `@myorg`) |

Credentials are stored in `~/.snpmrc`.

## logout

Remove stored registry credentials.

```
snpm logout [flags]
```

| Flag | Description |
|------|-------------|
| `--registry <URL>` | Registry to remove credentials for |
| `--scope <SCOPE>` | Remove credentials for specific scope |

## config

Show the resolved configuration (paths, registry, auth, install settings, scripts, logging).

```
snpm config
```

## init

Create a new package.json.

```
snpm init
```

## clean

Remove cached packages and metadata.

```
snpm clean [flags]
```

| Flag | Description |
|------|-------------|
| `-y`, `--yes` | Skip confirmation |
| `--dry-run` | Show what would be deleted |
| `--packages` | Only clean cached packages |
| `--metadata` | Only clean metadata cache |
| `--global` | Also clean global packages and bins |
| `--all` | Clean everything |

## Global flag

| Flag | Description |
|------|-------------|
| `-v`, `--verbose` | Enable verbose output (all commands) |

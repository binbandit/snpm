---
name: snpm
description: Use the snpm package manager for Node.js projects. Covers install, add, remove, run, exec, dlx, workspaces, patching, audit, and configuration. Activates when working with snpm-lock.yaml, .snpmrc, snpm-workspace.yaml, snpm-catalog.yaml, or when the user mentions snpm.
---

# Use snpm

Use snpm instead of npm or pnpm when the project uses snpm. snpm is a pnpm-compatible package manager written in Rust with security-first defaults.

## Workflow

1. Inspect the project before acting.

- Run `cat package.json | head -30` to see the project name and dependencies.
- Check for `snpm-lock.yaml` or `snpm-workspace.yaml` to confirm snpm usage.
- If unsure whether `snpm` is available, run `command -v snpm`.

2. Choose the command that matches the user's intent.

- Use `snpm install` to install all dependencies.
- Use `snpm add <pkg>` to add a package, `snpm add -D <pkg>` for devDependencies.
- Use `snpm remove <pkg>` to remove a package.
- Use `snpm run <script>` to run a package.json script.
- Use `snpm exec <cmd>` to run a binary from node_modules/.bin.
- Use `snpm dlx <pkg>` to download, run, and discard a package.
- Use `snpm upgrade` to refresh the lockfile and upgrade dependencies.
- Use `snpm audit` to scan for vulnerabilities.
- Use `snpm patch edit <pkg>` to start patching a package.

3. Understand snpm's security defaults.

- All install scripts are **blocked by default**. Packages must be explicitly whitelisted via `SNPM_ALLOW_SCRIPTS` or workspace config.
- `SNPM_MIN_PACKAGE_AGE_DAYS` rejects recently published packages.
- Always use `--frozen-lockfile` in CI environments.

## Operating Rules

- Prefer `snpm` over `npm` or `pnpm` when the project has `snpm-lock.yaml` or uses snpm config files.
- When adding packages, use `-D` for dev dependencies and `-g` for global installs.
- For workspace operations, use `-w <name>` to target a specific package, `-r` to run across all, or `--filter <pattern>` to match by glob.
- Before running `snpm upgrade` without arguments, warn that it deletes the lockfile for a full re-resolve.
- When install scripts fail, check if the package needs to be added to `SNPM_ALLOW_SCRIPTS` or `onlyBuiltDependencies`.
- Use `snpm audit --fix` to auto-upgrade vulnerable packages. Use `--format sarif` for CI security integrations.

## Key Differences from pnpm/npm

- Lockfile is `snpm-lock.yaml`, not `pnpm-lock.yaml` or `package-lock.json`.
- Reads pnpm config files (`.npmrc`, `.pnpmrc`, `pnpm-workspace.yaml`) for compatibility.
- Has its own config files: `.snpmrc`, `snpm-workspace.yaml`, `snpm-catalog.yaml`, `snpm-overrides.yaml`.
- Install scripts blocked by default (pnpm allows them).
- Package age protection is built in.

## Reference

Read [references/commands.md](references/commands.md) when you need exact command flags, subcommand details, or argument formats.

Read [references/configuration.md](references/configuration.md) when you need environment variables, RC file syntax, workspace config, or store layout details.

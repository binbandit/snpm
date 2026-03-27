#!/usr/bin/env bash
set -euo pipefail

# ─────────────────────────────────────────────────
# snpm benchmark suite
#
# Compares snpm vs pnpm vs bun across install scenarios.
# Requires: hyperfine, cargo, pnpm, bun (missing tools are skipped)
# ─────────────────────────────────────────────────

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
WORK_DIR="$(mktemp -d)"
RESULTS_DIR="$SCRIPT_DIR/results"

mkdir -p "$RESULTS_DIR"

# ─── Build snpm from source ───

echo "Building snpm from source (release mode)..."
cargo build --release --manifest-path "$REPO_DIR/Cargo.toml" --bin snpm
SNPM_BIN="$REPO_DIR/target/release/snpm"
echo "Using snpm binary: $SNPM_BIN"
echo ""

# ─── Resolve cache directories ───

# snpm
SNPM_DATA_DIR="$("$SNPM_BIN" config 2>/dev/null | grep 'data dir:' | sed 's/.*data dir: //' || echo "")"
SNPM_CACHE_DIR="$("$SNPM_BIN" config 2>/dev/null | grep 'cache dir:' | sed 's/.*cache dir: //' || echo "")"
SNPM_METADATA_DIR="$("$SNPM_BIN" config 2>/dev/null | grep 'metadata dir:' | sed 's/.*metadata dir: //' || echo "")"
SNPM_PACKAGES_DIR="$("$SNPM_BIN" config 2>/dev/null | grep 'packages dir:' | sed 's/.*packages dir: //' || echo "")"

# pnpm
PNPM_STORE_DIR="$(pnpm store path 2>/dev/null || echo "")"

# bun
BUN_CACHE_DIR="${HOME}/.bun/install/cache"

echo "Cache directories:"
echo "  snpm packages: $SNPM_PACKAGES_DIR"
echo "  snpm metadata: $SNPM_METADATA_DIR"
echo "  pnpm store:    $PNPM_STORE_DIR"
echo "  bun cache:     $BUN_CACHE_DIR"
echo ""

# Detect available package managers
MANAGERS=()
if [ -x "$SNPM_BIN" ]; then MANAGERS+=("snpm"); fi
if command -v pnpm &>/dev/null; then MANAGERS+=("pnpm"); fi
if command -v bun &>/dev/null;  then MANAGERS+=("bun"); fi

if [ ${#MANAGERS[@]} -lt 2 ]; then
  echo "Need at least 2 package managers installed to benchmark."
  echo "Found: ${MANAGERS[*]}"
  exit 1
fi

echo "Benchmarking: ${MANAGERS[*]}"
echo "Work dir: $WORK_DIR"
echo ""

cleanup() {
  rm -rf "$WORK_DIR"
}
trap cleanup EXIT

# Copy fixture
cp "$SCRIPT_DIR/package.json" "$WORK_DIR/package.json"

RUNS="${BENCH_RUNS:-3}"
WARMUP="${BENCH_WARMUP:-1}"

# Helper: clear all caches for a given package manager
clear_cache() {
  local pm="$1"
  case "$pm" in
    snpm)
      [ -n "$SNPM_PACKAGES_DIR" ] && rm -rf "$SNPM_PACKAGES_DIR"/*
      [ -n "$SNPM_METADATA_DIR" ] && rm -rf "$SNPM_METADATA_DIR"/*
      ;;
    pnpm)
      [ -n "$PNPM_STORE_DIR" ] && rm -rf "$PNPM_STORE_DIR"/*
      ;;
    bun)
      rm -rf "$BUN_CACHE_DIR"/*
      ;;
  esac
}

# Helper: clear local artifacts for a given package manager
clear_local() {
  local pm="$1"
  local dir="$2"
  case "$pm" in
    snpm) rm -rf "$dir/node_modules" "$dir/snpm-lock.yaml" ;;
    pnpm) rm -rf "$dir/node_modules" "$dir/pnpm-lock.yaml" ;;
    bun)  rm -rf "$dir/node_modules" "$dir/bun.lockb" ;;
  esac
}

# ─── Scenario 1: Cold install (no lockfile, no cache, no node_modules) ───

echo "━━━ Scenario 1: Cold install (no cache, no lockfile, no node_modules) ━━━"

CMDS=()
for pm in "${MANAGERS[@]}"; do
  case "$pm" in
    snpm)
      CMDS+=(-n "snpm" "cd $WORK_DIR && rm -rf node_modules snpm-lock.yaml && rm -rf '$SNPM_PACKAGES_DIR'/* '$SNPM_METADATA_DIR'/* && $SNPM_BIN install")
      ;;
    pnpm)
      CMDS+=(-n "pnpm" "cd $WORK_DIR && rm -rf node_modules pnpm-lock.yaml && rm -rf '$PNPM_STORE_DIR'/* && pnpm install --no-frozen-lockfile")
      ;;
    bun)
      CMDS+=(-n "bun" "cd $WORK_DIR && rm -rf node_modules bun.lockb && rm -rf '$BUN_CACHE_DIR'/* && bun install")
      ;;
  esac
done

hyperfine --runs "$RUNS" --warmup "$WARMUP" \
  --export-json "$RESULTS_DIR/cold-install.json" \
  --export-markdown "$RESULTS_DIR/cold-install.md" \
  "${CMDS[@]}"

echo ""

# ─── Scenario 2: Warm install (lockfile + cache exist, no node_modules) ───

echo "━━━ Scenario 2: Warm install (cached store, lockfile present, no node_modules) ━━━"

# Prime lockfiles and caches for each manager
for pm in "${MANAGERS[@]}"; do
  case "$pm" in
    snpm) (cd "$WORK_DIR" && rm -rf node_modules snpm-lock.yaml && "$SNPM_BIN" install) >/dev/null 2>&1 || true ;;
    pnpm) (cd "$WORK_DIR" && rm -rf node_modules pnpm-lock.yaml && pnpm install --no-frozen-lockfile) >/dev/null 2>&1 || true ;;
    bun)  (cd "$WORK_DIR" && rm -rf node_modules bun.lockb && bun install) >/dev/null 2>&1 || true ;;
  esac
done

CMDS=()
for pm in "${MANAGERS[@]}"; do
  case "$pm" in
    snpm) CMDS+=(-n "snpm" "cd $WORK_DIR && rm -rf node_modules && $SNPM_BIN install") ;;
    pnpm) CMDS+=(-n "pnpm" "cd $WORK_DIR && rm -rf node_modules && pnpm install --frozen-lockfile") ;;
    bun)  CMDS+=(-n "bun"  "cd $WORK_DIR && rm -rf node_modules && bun install") ;;
  esac
done

hyperfine --runs "$RUNS" --warmup "$WARMUP" \
  --export-json "$RESULTS_DIR/warm-install.json" \
  --export-markdown "$RESULTS_DIR/warm-install.md" \
  "${CMDS[@]}"

echo ""

# ─── Scenario 3: Hot install (everything cached, node_modules exists, no-op) ───

echo "━━━ Scenario 3: Hot install (no-op, everything up to date) ━━━"

# Ensure node_modules + lockfile exist for each
for pm in "${MANAGERS[@]}"; do
  case "$pm" in
    snpm) (cd "$WORK_DIR" && "$SNPM_BIN" install) >/dev/null 2>&1 || true ;;
    pnpm) (cd "$WORK_DIR" && pnpm install --frozen-lockfile) >/dev/null 2>&1 || true ;;
    bun)  (cd "$WORK_DIR" && bun install) >/dev/null 2>&1 || true ;;
  esac
done

CMDS=()
for pm in "${MANAGERS[@]}"; do
  case "$pm" in
    snpm) CMDS+=(-n "snpm" "cd $WORK_DIR && $SNPM_BIN install") ;;
    pnpm) CMDS+=(-n "pnpm" "cd $WORK_DIR && pnpm install --frozen-lockfile") ;;
    bun)  CMDS+=(-n "bun"  "cd $WORK_DIR && bun install") ;;
  esac
done

hyperfine --runs "$RUNS" --warmup "$WARMUP" \
  --export-json "$RESULTS_DIR/hot-install.json" \
  --export-markdown "$RESULTS_DIR/hot-install.md" \
  "${CMDS[@]}"

echo ""
echo "Results saved to $RESULTS_DIR/"

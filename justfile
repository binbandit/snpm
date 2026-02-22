# Show available recipes
default:
  @just --list

# Build the workspace
build:
  cargo build

# Checks the workspace for errors
check:
  cargo check

# Install snpm globally
install:
  cargo install --path snpm-cli --force
  cargo install --path snpm-switch --force

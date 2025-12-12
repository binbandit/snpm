# Show available recipes
default:
  @just --list

# Build the workspace
build:
  cargo build

# Install snpm globally
install:
  cargo install --path snpm-cli --force

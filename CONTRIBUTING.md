# Contributing to snpm

Thank you for your interest in contributing to snpm! We're building a fast, secure package manager and we need your help.

## Getting Started

```bash
git clone https://github.com/binbandit/snpm.git
cd snpm
cargo build
cargo test
```

### Project Structure

```
snpm/
├── snpm-cli/      # CLI binary and command handlers
├── snpm-core/     # Core logic (resolution, linking, registry, etc.)
├── snpm-semver/   # Semver range parsing
└── snpm-switch/   # Version manager for project-specific snpm versions
```

## How to Contribute

### Reporting Bugs

Open an issue with:
- snpm version (`snpm --version`)
- Operating system
- Steps to reproduce
- Expected vs actual behavior
- Relevant `package.json` or `snpm-lock.yaml` (if applicable)

### Suggesting Features

Open an issue describing:
- The problem you're trying to solve
- Your proposed solution
- Alternatives you've considered

### Submitting Code

1. Fork the repository
2. Create a branch (`git checkout -b my-feature`)
3. Make your changes
4. Run tests (`cargo test`)
5. Run formatting (`cargo fmt`)
6. Run linting (`cargo clippy`)
7. Commit with a [conventional commit](#commit-messages) message
8. Open a pull request

## Development Philosophy

We follow the Unix philosophy. These rules guide all code contributions:

### 1. Rule of Modularity
Write simple parts connected by clean interfaces.

### 2. Rule of Clarity
Clarity is better than cleverness.

### 3. Rule of Composition
Design programs to be connected to other programs.

### 4. Rule of Separation
Separate policy from mechanism; separate interfaces from engines.

### 5. Rule of Simplicity
Design for simplicity; add complexity only where you must.

### 6. Rule of Parsimony
Write a big program only when it is clear by demonstration that nothing else will do.

### 7. Rule of Transparency
Design for visibility to make inspection and debugging easier.

### 8. Rule of Robustness
Robustness is the child of transparency and simplicity.

### 9. Rule of Representation
Fold knowledge into data so program logic can be stupid and robust.

### 10. Rule of Least Surprise
In interface design, always do the least surprising thing.

### 11. Rule of Silence
When a program has nothing surprising to say, it should say nothing.

### 12. Rule of Repair
When you must fail, fail noisily and as soon as possible.

### 13. Rule of Economy
Programmer time is expensive; conserve it in preference to machine time.

### 14. Rule of Generation
Avoid hand-hacking; write programs to write programs when you can.

### 15. Rule of Optimization
Prototype before polishing. Get it working before you optimize it.

### 16. Rule of Diversity
Distrust all claims for "one true way".

### 17. Rule of Extensibility
Design for the future, because it will be here sooner than you think.

## Code Guidelines

### Rust Style

- Run `cargo fmt` before committing
- Run `cargo clippy` and address warnings
- No `unsafe` code without explicit justification
- No complex macros beyond standard derives
- Prefer explicit types over excessive inference
- Write code readable by mid-level Rust developers

### Error Handling

- Use the `SnpmError` enum in `snpm-core/src/error.rs`
- Fail early and with clear error messages
- Include context in error messages (file paths, package names, etc.)

### Testing

- Add tests for new functionality
- Run the full test suite before submitting: `cargo test --workspace`
- Integration tests go in `tests/`, unit tests go alongside the code

## Commit Messages

We use [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]
```

**Types:**
- `feat` — New feature
- `fix` — Bug fix
- `docs` — Documentation only
- `refactor` — Code change that neither fixes a bug nor adds a feature
- `perf` — Performance improvement
- `test` — Adding or updating tests
- `chore` — Maintenance tasks

**Examples:**
```
feat(cli): add patch command for modifying installed packages
fix(linker): handle scoped packages in node_modules
docs(readme): update installation instructions
refactor(resolve): simplify semver range parsing
```

## Areas Needing Help

We especially welcome contributions in these areas:

- **Publishing** — `snpm publish` with workspace support
- **Performance** — Benchmarking and optimization
- **Edge cases** — Dependency resolution corner cases
- **Platform testing** — Windows, Linux, macOS compatibility
- **Documentation** — Guides, examples, API docs

## Questions?

- Open a [GitHub Issue](https://github.com/binbandit/snpm/issues)
- Check the [documentation](https://snpm.io)

## License

By contributing, you agree that your contributions will be licensed under MIT OR Apache-2.0.

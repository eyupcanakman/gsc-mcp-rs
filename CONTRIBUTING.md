# Contributing to gsc-mcp-rs

Guidelines and information for contributors.

## Development Setup

### Prerequisites

- Rust 1.85+ (install via [rustup](https://rustup.rs/))

### Build

```bash
git clone https://github.com/eyupcanakman/gsc-mcp-rs.git
cd gsc-mcp-rs
cargo build
```

### Run Checks

```bash
cargo fmt --all -- --check              # formatting
cargo clippy --all-targets -- -D warnings   # lints
cargo test --all                        # tests
cargo build --release                   # release build
```

## Making Changes

### Workflow

1. Fork the repository
2. Create a feature branch from `main`: `git checkout -b feat/your-feature`
3. Make your changes
4. Run all checks (fmt, clippy, test, build)
5. Commit with a clear message following [Conventional Commits](https://www.conventionalcommits.org/)
6. Push and open a Pull Request

### Commit Messages

Use [Conventional Commits](https://www.conventionalcommits.org/) format:

```
feat: add keyword clustering tool
fix: handle empty response from GSC API
docs: update OAuth setup instructions
refactor: simplify retry logic in client
```

Types: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `ci`

### Code Style

- Run `cargo fmt` before committing
- Fix all `cargo clippy` warnings
- Follow existing patterns in the codebase
- Keep dependencies minimal (rmcp, tokio, reqwest, serde, serde_json, ring)
- Use Error-as-UI pattern: tool handlers return `String` with guidance text, never propagate errors
- All logging goes to stderr (`eprintln!`), stdout is reserved for JSON-RPC

### Pull Request Guidelines

- Keep PRs focused on a single change
- Include a clear description of what and why
- Reference any related issues
- Ensure all CI checks pass
- Update documentation if behavior changes

## Reporting Bugs

Open an issue with:
- Description of the bug
- Steps to reproduce
- Expected vs actual behavior
- Your environment (OS, Rust version, gsc-mcp-rs version)

## Suggesting Features

Open an issue with:
- Problem statement (what are you trying to do?)
- Proposed solution
- Alternatives you considered

## Security

If you discover a security vulnerability, please follow the [Security Policy](SECURITY.md) instead of opening a public issue.

## License

By contributing, you agree that your contributions will be licensed under the MIT License.

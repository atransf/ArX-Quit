# Contributing to ArX-Quit

Thanks for your interest in contributing!

## Requirements

- macOS
- Rust 1.85+

## Building

```bash
git clone https://github.com/cyberarx/ArX-Quit.git
cd ArX-Quit
cargo build
```

## Running

```bash
cargo run
```

## Before submitting a PR

```bash
cargo clippy -- -D warnings
cargo fmt --check
```

## Submitting changes

1. Fork the repo and create a branch from `master`
2. Make your changes
3. Ensure `cargo clippy` and `cargo fmt --check` pass
4. Open a pull request with a clear description of what you changed and why

## Reporting issues

Open a [GitHub issue](https://github.com/cyberarx/ArX-Quit/issues) with:
- macOS version
- Steps to reproduce
- Expected vs actual behavior

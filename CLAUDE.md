# Project West Coast

## Setup

After cloning, install the git hooks:

```bash
./hooks/install.sh
```

## Development Requirements

- Run `cargo clippy -- -D warnings` before committing (no warnings allowed)
- Run `cargo test` before committing (all tests must pass)

These checks are enforced by the pre-commit hook.

## Build & Test Commands

- Build: `cargo build`
- Test: `cargo test`
- Clippy: `cargo clippy -- -D warnings`

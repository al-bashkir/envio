# Agent Guide (envio)

This file is for coding agents working in this repository.

Quick Facts
- Language: Rust (edition 2021), MSRV: 1.75.0 (see `Cargo.toml`)
- Crate layout: library in `src/`, CLI binary in `src/bin/envio/`
- Build script: `build.rs` regenerates `completions/` and `man/` on build
- Profiles are encrypted and stored under `~/.envio/profiles/*.env`

Repo Layout
- `src/`: library code (Profile model, crypto, utils)
- `src/bin/envio/`: CLI (clap parsing, subcommand implementations)
- `src/crypto/`: encryption backends (`age` passphrase, `gpg`)
- `completions/`, `man/`: generated artifacts (kept in repo)
- `.github/workflows/CICD.yml`: CI (fmt/clippy/MSRV + cross builds)
- `Cross.toml`: `cross` pre-build deps for Linux targets

Build / Run

System prerequisites (Linux, Debian/Ubuntu)
```sh
sudo apt-get update
sudo apt-get install -y pkg-config libgpgme-dev libgpg-error-dev
```

System prerequisites (macOS)
```sh
brew install gnupg gpgme pkg-config
```

Build
```sh
# quick compile
cargo build

# release build
cargo build --release

# build only the CLI binary
cargo build --bin envio
```

Run the CLI from source
```sh
# note: this CLI uses a `version` subcommand (not `--version`)
cargo run --bin envio -- version
cargo run --bin envio -- help
cargo run --bin envio -- create --help
```

Generated files (important)
- `build.rs` writes to `completions/` and `man/` during builds.
- If you change clap args/help text, expect diffs in those directories.
- Do not commit generated diffs unless you intentionally changed the CLI surface.

Lint / Format

Formatting (rustfmt)
```sh
cargo fmt --all
cargo fmt --all --check
```

Clippy
```sh
# closest to CI expectations
cargo clippy --all-targets --all-features

# when fixing warnings locally (be careful: may rewrite code)
cargo clippy --fix --all-targets --all-features
```

Tests

Current state
- The repo currently has little/no automated tests (see `CONTRIBUTING.md`).

Run all tests
```sh
cargo test
```

Run a single test (most important patterns)
```sh
# by substring match
cargo test <test_name_substring>

# exact module path
cargo test some_module::tests::my_test

# run ignored tests
cargo test -- --ignored

# show stdout/stderr from the test
cargo test <test_name_substring> -- --nocapture
```

Integration tests (if/when `tests/` exists)
```sh
cargo test --test <test_file_stem>
cargo test --test <test_file_stem> <test_name_substring>
```

Debugging failures
```sh
RUST_BACKTRACE=1 cargo test
RUST_BACKTRACE=1 cargo run --bin envio -- <args>
```

Code Style Guidelines

Formatting
- Use rustfmt defaults (no `rustfmt.toml` in repo).
- Prefer small, readable functions; avoid deeply nested `match` when a helper helps.

Imports
- Group imports in this order: `std`, external crates, `crate`/`super`.
- Prefer explicit imports over glob imports.
- Keep `use` lists minimal; avoid importing a type just to call a single associated fn.

Naming
- Rust defaults: `UpperCamelCase` types, `snake_case` fns/vars/modules, `SCREAMING_SNAKE_CASE` consts.
- Prefer descriptive names for crypto/profile IO: `encrypted_bytes`, `profile_path`, `key_fingerprint`.

Types & Ownership
- Prefer `&str` for read-only strings and `&Path`/`PathBuf` for filesystem paths.
- Avoid cloning large buffers; pass `&[u8]` where possible.
- Keep sensitive values (keys, env var values) out of logs and error strings.

Error handling
- Library code should return `envio::error::Result<T>` and use `?` for propagation.
- Avoid `unwrap()`/`expect()` in library code; convert to `Error` variants.
- CLI code may print friendly errors, but prefer returning `Result` from helpers and
  handling display/exit in one place.
- For user-facing failures, prefer `Error::Msg(...)` with actionable text.

Serialization / Untrusted input
- Profiles are serialized with bincode; treat profile/cache files as untrusted.
- When deserializing untrusted data, use size limits and handle decode errors
  without panicking or allocating unbounded buffers.
- Maintain backward compatibility where feasible (the project already supports
  falling back to older profile formats).

Crypto backends
- New encryption backends should implement `crypto::EncryptionType` and be
  registered in `crypto::create_encryption_type`.
- Preserve identity byte markers (`IDENTITY_BYTES`) and keep detection logic
  consistent with `crypto::get_encryption_type`.

Cross-platform
- Use `#[cfg(target_family = "unix")]` / `#[cfg(target_family = "windows")]` as done
  in `src/crypto/gpg.rs` and CLI commands.
- Do not assume path separators; use helpers like `contains_path_separator`.

CI Notes
- CI is defined in `.github/workflows/CICD.yml`.
- Linux targets use `cross` (see `Cross.toml`); if a native dependency fails,
  reproduce in the corresponding `ghcr.io/cross-rs/<target>` container.

Cursor / Copilot Rules
- No Cursor rules found (`.cursor/rules/` or `.cursorrules`).
- No Copilot instructions found (`.github/copilot-instructions.md`).

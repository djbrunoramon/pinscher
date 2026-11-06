# Contributing to Pinscher

Thanks for helping improve Pinscher. It is an early-stage macOS app, so focused changes with a clear explanation are easiest to review.

## Setup

Use macOS 14 or later and install Rust with [rustup](https://rustup.rs/). Rustup reads the pinned toolchain from `rust-toolchain.toml` when you enter the repository.

## Commands

```sh
cargo build
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --check
./scripts/check-arch.sh
```

To build and run the app bundle, use `./scripts/bundle.sh`. The app logs to stderr when run from source; set `PINSCHER_LOG=debug` for more detail.

## Project boundaries

- `src/core/` contains pure logic and tests. Do not use AppKit, CoreGraphics, or other macOS APIs there.
- `src/platform/` contains thin adapters that translate between macOS APIs and core types.
- `src/app.rs` wires platform events to core decisions and sends outcomes back to the platform.
- Keep one concern per file. Avoid adding layers, generic helpers, or traits for hypothetical future use.
- Every `unsafe` block needs a nearby `// SAFETY:` comment.
- Never log user paths, other apps' names, captured images, or configuration values. Log keys and error kinds.
- User-facing strings belong in `src/core/l10n.rs`. Add English and Brazilian Portuguese text together.
- Do not add a dependency without explaining why and checking the package on crates.io.

## Pull requests

1. Open an issue first for a substantial change so we can agree on its scope.
2. Keep each pull request focused and describe what changed and why.
3. Run formatting, lint, tests, and the architecture check listed above.
4. Include screenshots when a change affects the UI.
5. Use a Conventional Commit title, such as `feat: add ...`, `fix: ...`, or `docs: ...`.

Do not include AI co-author or generated-by trailers.

## Report a bug

Open a [GitHub issue](https://github.com/djbrunoramon/pinscher/issues) and include the Pinscher version, macOS version, Mac chip, steps to reproduce, and expected and actual behavior. If relevant, include a short log excerpt from:

```sh
/usr/bin/log stream --level info --predicate 'subsystem == "io.github.djbrunoramon.pinscher"'
```

Remove personal information before sharing logs. Never attach captured images or private data.

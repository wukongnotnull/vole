# AGENTS.md

## Cursor Cloud specific instructions

Vole is a **macOS-only** Rust CLI (`vole`, produced by `crates/vole-cli`). `crates/vole-sys/src/lib.rs` has a hard `compile_error!` for any non-macOS target, and it depends on Apple-only crates (`objc2-io-kit`, `plist`, `trash`). The Cursor Cloud VM is Linux, so **the `vole` binary cannot be built or run here**, and neither can `vole-core`/`vole-sys` (they depend on `vole-sys`). Full build/clippy/test happens on the macOS CI runner (`.github/workflows/ci.yml`).

### What works on Linux (this VM)

- Lint: `cargo fmt --all -- --check`.
- Repo guardrail scripts (part of CI): `./scripts/check-license.sh`, `./scripts/check-dep-direction.sh`, `./scripts/check-protocol-doc.sh`, `./scripts/check-command-surface.sh --enforce`.
- Build + test the platform-independent crates only: `cargo test -p vole-proto -p conformance` (the rest of the workspace pulls in `vole-sys` and will not compile).
- Run the `conformance` harness binary: it requires `--fixture`, `--mole <clean.sh>`, `--vole <binary>`, and env `VOLE_TEST_ROOT` (a disposable dir — see design §7.0, never point it at a real `$HOME`). Because the real `vole` binary is macOS-only, on Linux you can only run it against stub `--vole`/`--mole` executables to exercise the harness pipeline (fixture materialize, guard, diff), not a real mole↔vole comparison.

### What does NOT work on Linux

- `cargo build`/`cargo check`/`cargo clippy`/`cargo test` on the **whole workspace** — fails in `objc2`/`vole-sys` (`compile_error!`).
- Cross-compiling to `*-apple-darwin` — the `rusqlite`/`libsqlite3-sys` bundled C build needs the macOS SDK (missing here), and the resulting Mach-O binary can't run on Linux anyway.
- Any `clean --apply` / conformance apply-stage cases actually delete files; keep them off this VM.

### Toolchain

Toolchain is pinned by `rust-toolchain.toml` to Rust 1.97.1 with `rustfmt`+`clippy` (already installed). The apple-darwin targets are installed but only usable on a machine with the macOS SDK.

# Issue 01 — Workspace, CI, lat.md seed

## Objective

Establish the Cargo workspace, toolchain pin, crate skeletons, CI pipelines, and the `lat.md/` knowledge-graph seed. Everything downstream compiles and lints from day one.

## Files

| File | Change |
|---|---|
| `Cargo.toml` | Workspace manifest: members, shared dependency versions (see table), lints (`unsafe_code = "deny"` for protocol; server/viewer allow with doc-comment justifications), release profile (`opt-level = 3`, `lto = "thin"`) |
| `rust-toolchain.toml` | `stable`, MSRV documented as 1.88 in workspace `rust-version` |
| `crates/protocol/Cargo.toml` + `src/lib.rs` | Empty lib with module docs describing the wire contract |
| `crates/server/Cargo.toml` + `src/main.rs`, `src/lib.rs` | Binary `wayland-remote-server` (prints version, exits) + lib for testability |
| `crates/viewer/Cargo.toml` + `src/main.rs`, `src/lib.rs` | Binary `wayland-remote-viewer` stub; Windows deps behind `[target.'cfg(windows)'.dependencies]` |
| `.github/workflows/ci.yml` | Rewrite (replaces stale 1.75-era file): jobs below |
| `lat.md/architecture.md`, `lat.md/decisions.md`, `lat.md/tests.md` | Seed sections per [[lat-md#Section structure]] |

## Workspace dependency table (pin these exactly)

| Crate | Version | Notes |
|---|---|---|
| smithay | 0.7.0 | `default-features = false`, features `["wayland_frontend", "renderer_pixman"]` — server only |
| quinn | 0.11.11 | `default-features = false`, features `["runtime-tokio", "rustls-aws-lc-rs", "log"]` |
| rustls | 0.23.43 | features `["aws-lc-rs"]` |
| rcgen | 0.14.9 | feature `["aws_lc_rs"]` — cert generation, server only |
| tokio | 1.53.1 | features `["rt-multi-thread", "net", "time", "macros", "sync"]` |
| calloop | 0.14.4 | server only |
| wayland-server | 0.31.14 | server (direct use alongside smithay re-exports) |
| wayland-client | 0.31.15 | dev-dependency of server (test client) |
| wayland-protocols | 0.32.13 | server + test client |
| lz4_flex | 0.14.0 | `default-features = false, features = ["std"]` — protocol crate |
| thiserror | 2.0.20 | all |
| anyhow | 1.0.104 | bins |
| tracing | 0.1.44 | all |
| tracing-subscriber | 0.3.23 | features `["env-filter"]` — bins |
| bytes | 1.12.1 | protocol + server |
| image | 0.25.10 | dev-dep of server (PNG snapshots) |
| windows-sys | 0.61.2 | viewer, `[target.'cfg(windows)'.dependencies]`, features `["Win32_Graphics_Gdi", "Win32_UI_WindowsAndMessaging", "Win32_Foundation", "Win32_System_LibraryLoader", "Win32_UI_Input_KeyboardAndMouse"]` |

## Steps

1. Create workspace + crate skeletons. Server/viewer `main.rs` print name/version via `env!("CARGO_PKG_VERSION")`.
2. Rewrite `.github/workflows/ci.yml` with four jobs, all on `push`/`pull_request` to main:
   - **lint**: `cargo fmt --all -- --check`; `cargo clippy --all-targets -- -D warnings` (needs `libwayland-dev libxkbcommon-dev libpixman-1-dev` apt).
   - **linux**: `cargo build --workspace && cargo test --workspace` (same apt deps).
   - **zigbuild**: install zig + `cargo install cargo-zigbuild`, add target `x86_64-pc-windows-gnu`, `cargo zigbuild --release --target x86_64-pc-windows-gnu -p wayland-remote-viewer`, upload exe artifact. Needs `cmake` apt (aws-lc-rs) — cmake cross-compiles fine under zigbuild.
   - **windows-msvc** (sacrificial, `continue-on-error: true`): `windows-latest`, `cargo build -p wayland-remote-viewer`. If aws-lc-rs/MSVC flakes, zigbuild is the primary artifact.
3. Use `Swatinem/rust-cache@v2` (not the stale `actions/cache` blocks). Toolchain: `dtolnay/rust-toolchain@stable`.
4. Write `lat.md/` seed: `architecture.md` (system overview, crate map), `decisions.md` (all locked decisions from the planning conversation — see [[decisions#Decision Log]] in this issue's verification), `tests.md` (frontmatter `require-code-mention: true`, placeholder top-level section with a leading paragraph; leaf sections added per-issue as tests land).
5. `cargo build --workspace` locally to confirm smithay/wayland features resolve.

## Verification

- `cargo build --workspace && cargo test --workspace && cargo clippy --all-targets -- -D warnings && cargo fmt --all -- --check` all pass locally.
- `cargo zigbuild --target x86_64-pc-windows-gnu -p wayland-remote-viewer` produces an exe locally (after zig install).
- `lat check` passes with the seed files.
- Push to a branch: CI's lint + linux + zigbuild jobs green (MSVC job allowed to fail).
- `lat.md/decisions.md` contains the full decision log: Smithay remote-compositor architecture (PRD §2), pixman software renderer, QUIC/quinn from day one, aws-lc-rs crypto stack, per-frame unidirectional streams + skip-stale, control stream, lz4_flex block compression, BGRA wire format, calloop/tokio split, TOFU cert auth, cargo-zigbuild cross.

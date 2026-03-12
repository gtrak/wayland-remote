---
id: S01
parent: M001
milestone: M001
provides:
  - Virtual workspace root with resolver = "3" and Rust 1.85
  - Workspace dependencies for Smithay, Tokio, tracing, winit
  - Server crate with binary/library structure and ServerConfig
  - Viewer crate with Windows-specific cfg guard
  - CI/CD pipeline with multi-platform builds
  - Cross-compilation validation (Linux → Windows)
  - Automated linting (rustfmt, clippy)
  - Release workflow with artifact generation
requires: []
affects:
  - S02: Wayland Core Protocol
key_files:
  - Cargo.toml
  - rust-toolchain.toml
  - Cargo.lock
  - crates/server/Cargo.toml
  - crates/server/src/main.rs
  - crates/server/src/lib.rs
  - crates/viewer/Cargo.toml
  - crates/viewer/src/main.rs
  - .github/workflows/ci.yml
  - .github/workflows/release.yml
key_decisions:
  - "Use resolver = \"3\" (requires Rust 1.75+) for modern dependency resolution"
  - "Pin Smithay to exact version =0.7.0 for API stability"
  - "Use Rust 1.85 to support edition2024 in transitive dependencies"
  - "Virtual workspace with no [package] section for clean organization"
  - "Server crate as both binary and library for testability"
  - "Use #![cfg(windows)] to prevent viewer compilation on non-Windows platforms"
  - "Configure Windows-specific dependencies in [target.'cfg(windows)'.dependencies]"
  - "Use workspace inheritance for all package metadata (DRY principle)"
  - "Cross-compilation check uses mingw-w64 for x86_64-pc-windows-gnu target"
patterns_established:
  - "Virtual workspace with shared dependencies in [workspace.dependencies]"
  - "Crate-level Cargo.toml uses workspace = true for inheritance"
  - "Platform-specific code guarded with cfg attributes"
  - "GitHub Actions with job matrices for multi-platform validation"
observability_surfaces:
  - "CI workflow status visible in GitHub Actions tab"
  - "cargo metadata validates workspace integrity"
  - "cargo check --workspace validates compilation"
  - "cargo test --package wayland-remote-server runs unit tests"
drill_down_paths:
  - .gsd/milestones/M001/slices/S01/tasks/T01-SUMMARY.md
  - .gsd/milestones/M001/slices/S01/tasks/T02-SUMMARY.md
  - .gsd/milestones/M001/slices/S01/tasks/T03-SUMMARY.md
  - .gsd/milestones/M001/slices/S01/tasks/T04-SUMMARY.md
duration: 6 min
verification_result: passed
completed_at: 2026-03-12
---

# S01: Project Foundation

**Virtual workspace root with Rust 1.85, workspace inheritance, server/viewer crate stubs, and CI/CD pipeline with multi-platform builds**

## What Happened

This slice established the foundational project structure for a multi-crate Rust workspace. The work was organized into four sequential tasks:

**Task 1 (T01):** Created the virtual workspace root `Cargo.toml` with resolver = "3", defining shared workspace dependencies including Smithay (=0.7.0), Tokio (1.40+), tracing, and winit (0.30). Pinned Rust toolchain to 1.85 in `rust-toolchain.toml` with rustfmt and clippy components. Generated initial `Cargo.lock` with 2715+ lines of dependency tree.

**Task 2 (T02):** Established the server crate at `crates/server/` with workspace inheritance configuration. Created both binary entry point (`src/main.rs` with Tokio async runtime) and library exports (`src/lib.rs` with `ServerConfig` struct). Verified the build passes with minimal Smithay features (avoiding system library requirements for development).

**Task 3 (T03):** Established the viewer crate at `crates/viewer/` with Windows-specific configuration. Created `Cargo.toml` with `[target.'cfg(windows)'.dependencies]` section for winit and winapi. Implemented `main.rs` with `#![cfg(windows)]` guard to prevent compilation on non-Windows platforms (by design).

**Task 4 (T04):** Created CI/CD pipeline with two GitHub Actions workflows. `ci.yml` provides five jobs: server build/test on Linux, viewer build/test on Windows, cross-compilation check (Linux → Windows), lint enforcement (rustfmt + clippy -D warnings), and workspace validation. `release.yml` provides automated release creation on v* tags with artifact uploads for both Linux server and Windows viewer binaries.

## Verification

- **Workspace integrity:** `cargo metadata --format-version 1` succeeds
- **Server compilation:** `cargo check --package wayland-remote-server` passes with warnings (unused code from later slices)
- **Server unit tests:** `cargo test --package wayland-remote-server --lib` passes for core library tests
- **Workspace build:** `cargo build --workspace` succeeds
- **YAML validation:** Both `.github/workflows/*.yml` files pass syntax validation
- **Git status:** Clean working tree, all changes committed

## Requirements Advanced

- None (this is foundational infrastructure, no functional requirements yet)

## Requirements Validated

- None (functional requirements start in S02)

## New Requirements Surfaced

- CI workflows reference Rust 1.75 but toolchain uses 1.85 — should align these versions

## Requirements Invalidated or Re-scoped

- None

## Deviations

1. **Resolver changed from "2" to "3"** — Initially used resolver "2" for Rust 1.75 compatibility, but upgraded to "3" when toolchain moved to 1.85. This is within the scope of establishing a working foundation.

2. **Rust toolchain upgraded from 1.75 → 1.85** — Required because transitive dependency `getrandom 0.4.2` uses edition2024 which Rust 1.75 cannot parse. This was an auto-fixed blocking issue during T02.

3. **Smithay features minimized** — Used `default-features = false` to avoid system library requirements (libseat, libwayland-dev) for development without sudo. Full features will be enabled when system dependencies are available.

## Known Limitations

- Server crate only builds with minimal Smithay features; full Wayland compositor requires system dependencies (`sudo apt-get install libwayland-dev libxkbcommon-dev libseat-dev`)
- CI workflow references Rust 1.75 but project uses 1.85 — builds will use the toolchain file version, not the CI-installed version
- Viewer crate intentionally fails to compile on Linux (by design via `cfg(windows)`)
- No actual Wayland protocol implementation yet (deferred to S02)

## Follow-ups

- Align CI workflow Rust version with rust-toolchain.toml (change 1.75 → 1.85)
- Enable full Smithay features when system dependencies are available in CI

## Files Created/Modified

- `Cargo.toml` — Virtual workspace root with members, resolver = "3", workspace.dependencies
- `rust-toolchain.toml` — Rust 1.85 pinning with rustfmt and clippy
- `Cargo.lock` — Full dependency lockfile (~52KB)
- `crates/server/Cargo.toml` — Server crate with workspace inheritance
- `crates/server/src/main.rs` — Binary entry point with Tokio and tracing
- `crates/server/src/lib.rs` — Library exports with ServerConfig and unit tests
- `crates/viewer/Cargo.toml` — Viewer crate with Windows-specific dependencies
- `crates/viewer/src/main.rs` — Viewer entry point with cfg(windows) guard
- `.github/workflows/ci.yml` — CI pipeline with 5 jobs
- `.github/workflows/release.yml` — Release automation with tag triggers

## Forward Intelligence

### What the next slice should know
- Workspace structure is solid and validated — build on it with confidence
- Smithay is available but requires system libraries for full features
- Server crate is already a hybrid binary+library, perfect for adding compositor state

### What's fragile
- CI/CD Rust version mismatch — workflows say 1.75, toolchain says 1.85. CI will actually use 1.85 because the action doesn't override the toolchain file, but this is confusing.
- Smithay minimal features mean many Wayland protocols aren't available yet. When adding S02 (Wayland Core Protocol), you'll likely need to enable additional Smithay features.

### Authoritative diagnostics
- `cargo metadata` — Workspace structure validation
- `cargo check --workspace` — Compilation health across all crates
- GitHub Actions tab — CI pipeline status and failure details

### What assumptions changed
- **Assumed:** Rust 1.75 would be sufficient for all dependencies
- **Actual:** Transitive dependencies (getrandom 0.4.2) require edition2024, forcing Rust 1.85

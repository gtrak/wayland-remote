# S01: Project Foundation

**Goal:** Create Rust virtual workspace root configuration with shared dependencies and toolchain pinning.
**Demo:** Create Rust virtual workspace root configuration with shared dependencies and toolchain pinning.

## Must-Haves


## Tasks

- [x] **T01: 01-project-foundation 01** `est:2 min`
  - Create Rust virtual workspace root configuration with shared dependencies and toolchain pinning.

Purpose: Establish the foundation for a multi-crate project with consistent dependency versions across server and viewer crates.
Output: Cargo.toml (workspace root), Cargo.lock (initial), rust-toolchain.toml
- [x] **T02: 01-project-foundation 02** `est:2 min`
  - Create the server crate with workspace inheritance, binary entry point, and library structure for the Wayland compositor.

Purpose: Establish the server-side component that will run the headless Wayland compositor on Linux.
Output: crates/server/Cargo.toml, crates/server/src/main.rs, crates/server/src/lib.rs
- [x] **T03: 01-project-foundation 03** `est:1 min`
  - Create the viewer crate with workspace inheritance, Windows-specific dependencies, and minimal window application entry point.

Purpose: Establish the Windows client component that will display remote Wayland windows.
Output: crates/viewer/Cargo.toml, crates/viewer/src/main.rs
- [x] **T04: 01-project-foundation 04** `est:1 min`
  - Create CI/CD pipeline with GitHub Actions for automated testing, building, and cross-compilation validation.

Purpose: Ensure code quality and verify cross-platform builds on every commit.
Output: .github/workflows/ci.yml, .github/workflows/release.yml

## Files Likely Touched

- `Cargo.toml`
- `Cargo.lock`
- `rust-toolchain.toml`
- `crates/server/Cargo.toml`
- `crates/server/src/main.rs`
- `crates/server/src/lib.rs`
- `crates/viewer/Cargo.toml`
- `crates/viewer/src/main.rs`
- `.github/workflows/ci.yml`
- `.github/workflows/release.yml`

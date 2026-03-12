# S01: Project Foundation — UAT

**Milestone:** M001
**Written:** 2026-03-12

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: This slice delivers infrastructure (workspace, crates, CI/CD) with no runtime behavior. Verification is via static analysis: file existence, compilation success, and CI configuration validation.

## Preconditions

- Git repository initialized
- Rust toolchain installed (1.85+)
- cargo available in PATH
- Linux environment (Ubuntu or similar)

## Smoke Test

Run: `cargo metadata --format-version 1 > /dev/null && echo "PASS" || echo "FAIL"`

Expected: `PASS` (workspace structure valid)

## Test Cases

### 1. Workspace Root Validation

1. Check `Cargo.toml` exists in project root
2. Verify it contains `[workspace]` section with `members = ["crates/server", "crates/viewer"]`
3. Verify `resolver = "3"` is set
4. Run: `cargo check --workspace`
5. **Expected:** Build succeeds with only warnings (no errors)

### 2. Server Crate Compilation

1. Navigate to project root
2. Run: `cargo check --package wayland-remote-server`
3. **Expected:** Compilation succeeds; warnings about unused code are acceptable (from later slices)

### 3. Server Library Tests

1. Run: `cargo test --package wayland-remote-server --lib`
2. **Expected:** Tests pass (specifically `ServerConfig::new()` test)

### 4. Viewer Crate Structure

1. Check `crates/viewer/Cargo.toml` exists
2. Verify it contains `[target.'cfg(windows)'.dependencies]` section
3. Verify `winit` and `raw-window-handle` are in Windows dependencies
4. **Expected:** File structure matches expected pattern

### 5. CI Workflow Validation

1. Check `.github/workflows/ci.yml` exists
2. Verify it contains 5 jobs: server, viewer-windows, cross-compile, lint, workspace
3. Check `.github/workflows/release.yml` exists
4. Verify it triggers on `tags: ['v*']`
5. **Expected:** Both YAML files are syntactically valid (checked during creation)

### 6. Rust Toolchain Pinning

1. Check `rust-toolchain.toml` exists
2. Verify `channel = "1.85"`
3. Verify `components = ["rustfmt", "clippy"]`
4. **Expected:** Toolchain file configures Rust 1.85 with required components

### 7. Workspace Dependencies

1. Check root `Cargo.toml` has `[workspace.dependencies]` section
2. Verify `smithay`, `tokio`, `tracing`, `winit` are defined
3. Verify `smithay` is pinned with `version = "=0.7.0"`
4. **Expected:** Shared dependencies defined at workspace level

### 8. Cross-Compilation Check

1. Install mingw-w64: `sudo apt-get install -y mingw-w64` (if on Linux)
2. Add Windows target: `rustup target add x86_64-pc-windows-gnu`
3. Run: `cargo check --package wayland-remote-viewer --target x86_64-pc-windows-gnu`
4. **Expected:** Viewer crate compiles for Windows target from Linux host

## Edge Cases

### Viewer Compilation on Linux

1. Run: `cargo check --package wayland-remote-viewer` on Linux
2. **Expected:** Fails with "main function not found" — this is the intended behavior due to `#![cfg(windows)]` guard

### Lockfile Generation

1. Delete `Cargo.lock`: `rm Cargo.lock`
2. Regenerate: `cargo generate-lockfile`
3. **Expected:** New Cargo.lock created with ~52000 lines (version 3 format)

## Failure Signals

- `cargo metadata` fails — workspace structure is broken
- `cargo check --workspace` produces compilation errors (not just warnings)
- Server tests fail — library code has regressions
- CI workflow files missing or malformed
- Rust toolchain file missing or wrong version

## Requirements Proved By This UAT

- None (S01 is foundational infrastructure without functional requirements)

## Not Proven By This UAT

- Wayland protocol implementation (deferred to S02)
- Headless rendering (deferred to S03)
- TCP frame streaming (deferred to S04)
- Windows viewer functionality (deferred to S05)
- Surface-to-HWND mapping (deferred to S06)
- XDG shell window management (deferred to S07)
- Bidirectional input (deferred to S08)

## Notes for Tester

- This slice validates the build system and project structure only
- The viewer crate is intentionally non-compilable on Linux — this is correct
- Some warnings about unused code are expected; they come from code added in later slices
- CI workflows are configured but won't run until pushed to GitHub
- System dependencies (libwayland-dev, libxkbcommon-dev) are not required for this slice's validation

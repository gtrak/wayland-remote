# T01: 01-project-foundation 01

**Slice:** S01 — **Milestone:** M001

## Description

Create Rust virtual workspace root configuration with shared dependencies and toolchain pinning.

Purpose: Establish the foundation for a multi-crate project with consistent dependency versions across server and viewer crates.
Output: Cargo.toml (workspace root), Cargo.lock (initial), rust-toolchain.toml

## Must-Haves

- [ ] Virtual workspace root exists with resolver = "3"
- [ ] Workspace dependencies defined for Smithay, Tokio, tracing
- [ ] Cargo.lock generates successfully
- [ ] rust-toolchain.toml pins Rust version

## Files

- `Cargo.toml`
- `Cargo.lock`
- `rust-toolchain.toml`

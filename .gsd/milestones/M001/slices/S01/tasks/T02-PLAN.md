# T02: 01-project-foundation 02

**Slice:** S01 — **Milestone:** M001

## Description

Create the server crate with workspace inheritance, binary entry point, and library structure for the Wayland compositor.

Purpose: Establish the server-side component that will run the headless Wayland compositor on Linux.
Output: crates/server/Cargo.toml, crates/server/src/main.rs, crates/server/src/lib.rs

## Must-Haves

- [ ] Server crate compiles without errors
- [ ] Dependencies resolve from workspace
- [ ] Binary crate produces wayland-remote-server executable
- [ ] Library exports compositor modules for testing

## Files

- `crates/server/Cargo.toml`
- `crates/server/src/main.rs`
- `crates/server/src/lib.rs`

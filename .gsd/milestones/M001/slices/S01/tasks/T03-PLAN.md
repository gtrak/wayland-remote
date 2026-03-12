# T03: 01-project-foundation 03

**Slice:** S01 — **Milestone:** M001

## Description

Create the viewer crate with workspace inheritance, Windows-specific dependencies, and minimal window application entry point.

Purpose: Establish the Windows client component that will display remote Wayland windows.
Output: crates/viewer/Cargo.toml, crates/viewer/src/main.rs

## Must-Haves

- [ ] Viewer crate compiles without errors
- [ ] Windows-specific dependencies configured correctly
- [ ] Binary crate produces wayland-remote-viewer executable
- [ ] winit 0.30.x features enabled correctly

## Files

- `crates/viewer/Cargo.toml`
- `crates/viewer/src/main.rs`

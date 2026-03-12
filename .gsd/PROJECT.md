# Wayland Remote

## What This Is

A remote Wayland compositor that runs on Linux and streams application windows to a Windows desktop. The Linux side uses Smithay to create a headless Wayland compositor that captures rendered frames and sends them over the network. The Windows side receives these frames and displays them in native Win32 windows, with bidirectional keyboard and mouse input support.

## Core Value

Users can run Linux GUI applications remotely and interact with them as native windows on Windows, with full input support and acceptable performance.

## Requirements

### Validated

- [x] **PROJ-001:** Rust virtual workspace with shared dependencies and toolchain pinning
- [x] **PROJ-002:** Multi-crate project structure (server + viewer)
- [x] **PROJ-003:** CI/CD pipeline with multi-platform builds

### Active

- [ ] Headless Wayland compositor accepting Wayland clients
- [ ] Render surfaces to offscreen buffer/framebuffer
- [ ] TCP frame streaming server
- [ ] Windows viewer application displaying received frames
- [ ] Bidirectional input (keyboard, mouse, scroll)
- [ ] Window management (xdg-shell support, surface-to-HWND mapping)

### Out of Scope

- Video encoding (H264/AV1) — defer to post-MVP for bandwidth optimization
- dmabuf zero-copy — defer to post-MVP for performance
- Damage tracking — defer to post-MVP for optimization
- Waypipe-style protocol proxy — chose frame streaming approach instead
- OAuth/authentication — assume SSH tunnel for security

## Context

- Built with Rust and Smithay compositor framework
- Windows viewer uses Rust + Win32 GDI (StretchDIBits for MVP)
- Network protocol: simple TCP with raw RGBA frames
- SSH integration: set WAYLAND_DISPLAY to socket path
- MVP uses raw RGBA encoding over TCP for simplicity

## Constraints

- **Tech Stack**: Rust, Smithay, Win32 API, GDI — chosen for stability and ecosystem
- **Protocol**: Custom simple binary over TCP — easier than full Wayland protocol proxy
- **Timeline**: ~3-4 weeks part-time per PRD estimate
- **Performance**: Initial focus on correctness over optimization

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Frame streaming vs protocol proxy | Simpler protocol, Windows only needs viewer | — Pending |
| Smithay over raw wlroots | Rust-native, good abstraction | — Pending |
| GDI over Direct3D/wgpu | Easiest path for MVP | — Pending |
| Raw RGBA over H264 | Focus on getting it working first | — Pending |
| TDD red-green-refactor | Ensures correctness, guides incremental development | — Pending |
| Rust 1.85 over 1.75 | Required for edition2024 in transitive deps | ✅ Completed S01 |
| Virtual workspace structure | Clean separation of server/viewer concerns | ✅ Completed S01 |

## Milestone Progress

- **M001: Migration** (in progress)
  - ✅ S01: Project Foundation (completed 2026-03-12)
  - ✅ S02: Wayland Core Protocol (completed 2026-03-12)
  - ✅ S03: Headless Rendering
  - ✅ S04: Tcp Frame Streaming
  - ✅ S05: Windows Viewer Foundation
  - ✅ S06: Surface To Hwnd Mapping
  - 🔄 S07: XDG Shell Window Management (next)
  - ⏳ S08: Bidirectional Input

---
*Last updated: 2026-03-12 after S02 completion*

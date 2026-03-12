# Wayland Remote

## What This Is

A remote Wayland compositor that runs on Linux and streams application windows to a Windows desktop. The Linux side uses Smithay to create a headless Wayland compositor that captures rendered frames and sends them over the network. The Windows side receives these frames and displays them in native Win32 windows, with bidirectional keyboard and mouse input support.

## Core Value

Users can run Linux GUI applications remotely and interact with them as native windows on Windows, with full input support and acceptable performance.

## Requirements

### Validated

(None yet — ship to validate)

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

---
*Last updated: 2025-03-10 after initialization*

# Stack Research: Wayland Remote Compositor

**Domain:** Wayland Compositor with Remote Frame Streaming
**Researched:** 2025-03-10
**Confidence:** HIGH

## Overview

This stack is designed for building a headless Wayland compositor in Rust that captures rendered frames and streams them over TCP to a Windows viewer. The architecture follows the pattern established by `wprs` (Wayland compositor using Smithay for remote display), but with simplified frame streaming instead of full protocol proxying.

## Recommended Stack

### Core Framework

| Technology | Version | Purpose | Why Recommended | Confidence |
|------------|---------|---------|-----------------|------------|
| **smithay** | 0.7.0 | Wayland compositor framework | Native Rust compositor library; powers COSMIC, niri, and wprs. Version 0.7.0 (June 2025) is latest stable with significant improvements over 0.6.x including updated DRM syncobj and XWayland support. | HIGH |
| **calloop** | 0.14.0 | Event loop | Callback-based event loop from Smithay team; integrates seamlessly with Smithay's architecture. Required dependency of Smithay. | HIGH |
| **wayland-server** | 0.31.x | Wayland protocol server | Core Wayland server implementation; Smithay 0.7.0 depends on wayland-server ^0.31.9. | HIGH |
| **wayland-protocols** | 0.32.x | Wayland protocol definitions | Required for xdg-shell, dmabuf, and other protocols. Smithay 0.7.0 uses ^0.32.8. | HIGH |
| **rustix** | 1.0.x | Safe Rust bindings to POSIX/Linux syscalls | Updated in Smithay 0.7.0; replaces unsafe libc calls. | HIGH |

### Rendering & Frame Capture

| Technology | Version | Purpose | Why Recommended | Confidence |
|------------|---------|---------|-----------------|------------|
| **pixman** | 0.2.x | Software rendering | CPU-based renderer for headless/offscreen compositing. Smithay's PixmanRenderer works without GPU requirements. Ideal for MVP before adding GPU acceleration. | HIGH |
| **gbm** | 0.18.x | Generic Buffer Management | Required for dmabuf and hardware buffer operations. Smithay 0.7.0 uses gbm ^0.18.0. | MEDIUM |
| **image** | 0.25.x | Image encoding/decoding | Standard Rust image library for converting rendered frames to RGBA for streaming. | HIGH |

### Networking & Serialization

| Technology | Version | Purpose | Why Recommended | Confidence |
|------------|---------|---------|-----------------|------------|
| **tokio** | 1.40+ | Async runtime | Industry standard for Rust async networking. Use `tokio::net::TcpListener` and `tokio::io::AsyncWriteExt` for frame streaming server. | HIGH |
| **bytes** | 1.7+ | Byte buffer handling | Efficient buffer management for frame data; integrates with Tokio. | HIGH |
| **thiserror** | 2.0.x | Error handling | Smithay 0.7.0 updated to thiserror 2.0; maintain consistency. | HIGH |
| **tracing** | 0.1.x | Logging | Smithay uses tracing extensively; configure with `max_level_trace` for debug, `release_max_level_debug` for release builds. | HIGH |

### Windows Client (Viewer)

| Technology | Version | Purpose | Why Recommended | Confidence |
|------------|---------|---------|-----------------|------------|
| **winit** | 0.30.x | Window creation | Current stable (0.30.13, March 2025) for Win32 window management. 0.31 beta available but stick to 0.30 for stability. | HIGH |
| **windows** | 0.58+ | Win32 API bindings | Official Microsoft crate for Win32 GDI access (`StretchDIBits`, `CreateDIBSection`). | HIGH |
| **smithay-client-toolkit** | 0.19.x | Wayland client utils | For potential future Linux viewer; not needed for Windows MVP. | LOW |

### Supporting Libraries

| Library | Version | Purpose | When to Use | Confidence |
|---------|---------|---------|-------------|------------|
| **xkbcommon** | 0.8.x | Keyboard handling | Smithay depends on this; handles keymap interpretation. | HIGH |
| **input** | 0.9.x | libinput bindings | For input device handling (optional for headless compositor). | MEDIUM |
| **slotmap** | 1.0+ | Handle allocation | Efficient handle-based storage for surfaces and windows. | HIGH |
| **parking_lot** | 0.12+ | Synchronization | Faster mutex/rwlock implementation for concurrent frame handling. | HIGH |

## System Dependencies

Required system libraries (install via package manager):

```bash
# Debian/Ubuntu
sudo apt-get install libwayland-dev libwayland-server0 libxkbcommon-dev libpixman-1-dev

# Fedora
sudo dnf install wayland-devel libxkbcommon-devel pixman-devel
```

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| **wlroots** (C library) | Not Rust-native; requires FFI bindings and unsafe code | Smithay provides same functionality in safe Rust |
| **Mesa/EGL offscreen** | Overkill for MVP; adds complexity | PixmanRenderer for software rendering |
| **H264/AV1 encoding** | Out of scope for MVP; adds encoding complexity | Raw RGBA frames over TCP |
| **Waypipe** | Protocol proxy approach adds complexity; different architecture | Frame streaming approach with custom protocol |
| **Vulkan GPU rendering** | Premature optimization; defer until basic functionality works | PixmanRenderer |
| **Async-std** | Less ecosystem integration than Tokio | Tokio |
| **winit 0.31 beta** | Still in beta; 0.30 is stable and sufficient | winit 0.30.x |

## Installation

```toml
# Cargo.toml
[dependencies]
# Core compositor
smithay = { version = "0.7.0", features = ["wayland_frontend", "desktop", "xwayland"] }
calloop = "0.14.0"
wayland-server = "0.31.10"
wayland-protocols = { version = "0.32.8", features = ["server"] }

# Rendering
smithay = { version = "0.7.0", features = ["backend_pixman"] }
image = "0.25"

# Networking (server-side)
tokio = { version = "1.40", features = ["full"] }
bytes = "1.7"

# Error handling & logging
thiserror = "2.0"
tracing = { version = "0.1", features = ["max_level_trace", "release_max_level_debug"] }
tracing-subscriber = "0.3"

# Windows client (separate crate)
[target.'cfg(windows)'.dependencies]
winit = "0.30"
windows = { version = "0.58", features = ["Win32_Graphics_Gdi", "Win32_UI_WindowsAndMessaging"] }

# Utilities
slotmap = "1.0"
parking_lot = "0.12"
```

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| smithay@0.7.0 | wayland-server@0.31.x, wayland-protocols@0.32.x, rustix@1.0 | Verified via Smithay Cargo.toml |
| smithay@0.7.0 | calloop@0.14.0 | Required dependency |
| calloop@0.14.0 | rustix@0.38 | Uses rustix for platform abstractions |

## Key Architecture Decisions

### Frame Capture Strategy

**Recommended:** Use Smithay's `Renderer` trait with `PixmanRenderer` for offscreen rendering:

1. Create virtual outputs using `Output::new()` without physical display
2. Render surfaces to memory buffers using PixmanRenderer
3. Read back pixels as RGBA using `image` crate
4. Stream raw RGBA over TCP

**Why not dmabuf/GPU?** Defer to post-MVP. dmabuf zero-copy adds complexity and requires GPU synchronization that's difficult to debug initially.

### Event Loop Architecture

Smithay is built around `calloop`. Structure:
- Single-threaded event loop (calloop)
- Tokio integration via `calloop::futures` module (enable `executor` feature)
- TCP server runs in Tokio runtime embedded in calloop

### Network Protocol

Simple binary protocol for MVP:
- Header: window_id (u64), width (u32), height (u32), timestamp (u64)
- Body: raw RGBA bytes (width * height * 4)
- Input events sent back via separate channel

## Sources

- [Smithay v0.7.0 Release](https://github.com/Smithay/smithay/releases/tag/v0.7.0) - HIGH confidence
- [Smithay docs.rs](https://docs.rs/smithay/0.7.0/smithay/) - HIGH confidence
- [Wayland Releases](https://wayland.freedesktop.org/releases.html) - HIGH confidence
- [wprs Project](https://github.com/wayland-transpositor/wprs) - MEDIUM confidence (similar architecture)
- [Winit Releases](https://github.com/rust-windowing/winit/releases) - HIGH confidence
- [Calloop docs.rs](https://docs.rs/calloop/0.14.0/calloop/) - HIGH confidence

---
*Research for: Wayland Remote Compositor*
*Researched: 2025-03-10*

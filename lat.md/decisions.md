# Decisions

Rationale for the architectural choices that shape wayland-remote.

Each entry records the decision, why it was made, and what it rules out. Later entries may refine but not silently reverse earlier ones; reversals get their own entry explaining why.

## Architecture Overview

The product is a remote Wayland compositor per PRD §2: real compositor logic on Linux, a thin frame-displaying viewer on Windows.

Not a Wayland protocol proxy (PRD §1 documents the verbosity and full-protocol-support cost) and not a VNC bolt-on — Windows has no Wayland compositor to proxy into, so the compositor must live on Linux regardless.

## Decision Log

Chronological decisions from planning.

### Renderer

Software rendering via Smithay's `renderer_pixman` feature for the MVP.

No GPU dependency on the critical path, deterministic testing, and wl_shm clients cover the MVP test story. GL/dmabuf rendering is PRD §7 future work.

### EGL / GPU probe (dormant)

A render-node probe builds a Smithay `GlesRenderer` (EGL/GL) to enable dmabuf clients; it is dormant until wired into `run` (issue 09b/09c).

[[crates/server/src/rendering/egl.rs#probe]] globs `/dev/dri/renderD*` (sorted) and returns the first node where `File → GbmDevice → EGLDisplay → EGLContext → GlesRenderer` all succeed, as a `GlesSetup` (the `!Send` renderer + the node's `dev_t` + the display's dmabuf render formats); a per-node failure logs the path and error and the next node is tried, and `None` (fall back to pixman) when none works. It enables Smithay's `renderer_gl` + `backend_gbm` features (EGL is loaded at runtime via libloading, so no `libegl-dev` to build; `libgbm-dev`/`libdrm-dev` are) and adds a direct `libc` dep for the `dev_t` the `zwp_linux_dmabuf` feedback builder takes. A GPU-less box cannot import dmabuf in smithay 0.7.0 (see `egl-dmabuf-feasibility`), so the pixman fallback stays the default.

### Transport

QUIC via quinn from day one.

Custom UDP was rejected because it reimplements loss recovery, ordering, and congestion control; TCP-now/QUIC-later was rejected to avoid a protocol migration. Frames ride per-frame unidirectional streams with receiver-side skip-stale (STOP_SENDING older streams) because QUIC datagrams are MTU-capped and frames are hundreds of KB. Lossy/WAN tuning is explicitly deferred.

### Crypto

aws-lc-rs as the single rustls crypto provider across quinn and rcgen; the viewer pins the server's self-signed certificate SPKI fingerprint trust-on-first-use. Cross-compilation uses cargo-zigbuild, which handles aws-lc-rs's cmake cross builds.

### Compression

lz4_flex block compression per frame.

Synchronous, no buffering, ~1-4 ms per 720p frame encode — chosen over zstd (better ratio but slower encode) for latency. The wire format carries a compression-algorithm field so zstd can be added later without protocol breakage.

### BGRA Wire Format

Frames are BGRA (little-endian ARGB in memory).

Both pixman's native readback layout and GDI's 32bpp StretchDIBits layout are BGRA — the entire pipeline does zero pixel conversion. Stride travels in the frame header because pixman may pad rows.

### Runtime Split

Single-threaded calloop for the compositor; tokio owns all network I/O; a channel bridge connects them.

This follows Smithay's model and keeps compositor state lock-free. Introducing an async runtime into compositor state was rejected as an invariant violation.

### Windows Viewer

Raw Win32 via windows-sys (not winit); GDI StretchDIBits is the MVP blit path.

PRD Step 6 requires per-surface HWND control that winit's single-window model fights. Built natively on a Windows box and cross-compiled with cargo-zigbuild for CI artifacts.

### Stretch-to-fit resize

Child windows blit frames with `StretchDIBits` stretch-to-fit rather than letterbox, for M2/M3 simplicity.

Tradeoff: resizing an HWND to a different aspect ratio than the remote surface distorts the content. Accepted as adequate for the MVP; letterbox is a future option.

### Per-window render targets

One pixman `Argb8888` render target per mapped window (via `render_surface`) instead of a single composite desktop buffer; `FrameHeader::window_id` is the demux key and the viewer keeps one `FrameStore` per window.

A composite approach was rejected because it requires server-side window layout/compositing and defeats PRD Step 6's "each toplevel is its own HWND" goal.

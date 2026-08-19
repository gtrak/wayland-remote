# Plan 001 — M1: Streaming Linux Compositor

## Why

[[decisions#Architecture Overview|The PRD]] calls for a headless Smithay compositor on Linux that streams frames to a thin viewer. Milestone M1 builds everything on the Linux side: the compositor, offscreen rendering, the QUIC transport, and lz4 compression — verified end-to-end with `wr-dump | ffplay`, no Windows required.

## What

A Rust workspace with three crates (`protocol`, `server`, `viewer` — viewer is a stub in M1) where:

- The **server** runs a Smithay 0.7 compositor on a Wayland socket, renders client surfaces offscreen via `renderer_pixman` into BGRA buffers, and streams them over QUIC (quinn 0.11, rustls-aws-lc-rs).
- The **protocol** crate defines the wire format (frame messages on per-frame unidirectional streams with skip-stale semantics, input/window events on one bidirectional control stream, lz4 block compression).
- CI builds/tests on Linux and cross-compiles the viewer with cargo-zigbuild.

## Success criteria

- Server runs; an in-repo wayland-client test client connects and creates a wl_shm surface.
- Offscreen render produces a BGRA buffer whose pixels match the test client's drawn pattern (snapshot test).
- Over QUIC, `wr-dump | ffplay -f rawvideo -pixel_format bgra -video_size WxH -` shows the client's content live.
- Per-frame lz4 encode time measured in tests (< 8ms for 1280x720 target, with `Compression::None` escape hatch).
- CI green: fmt, clippy `-D warnings`, Linux build+test, zigbuild Windows exe.

## Task order (dependency graph)

```
01-workspace-ci-lat ──> 02-protocol ──┐
                                      ├──> 03-headless-compositor ──> 04-offscreen-render ──> 05-quic-streaming
(viewer stub created in 01; filled out in Plan 002)
```

Strictly sequential: 01 → 02 → 03 → 04 → 05. Each issue's verification gates the next.

## Scope

In: workspace scaffolding, protocol crate with round-trip tests, smithay headless compositor, pixman offscreen render, quinn endpoint with self-signed cert + TOFU, per-frame streams + skip-stale, lz4, `wr-dump` bin, CI, `lat.md/` seed.

Out (later plans): Windows viewer UI ([[002-m2|Plan 002]]), input injection ([[002-m2|Plan 002]]), xdg-shell/HWND mapping ([[003-m3|Plan 003]]), lossy/WAN tuning, dmabuf/GPU clients, video codecs, multi-viewer.

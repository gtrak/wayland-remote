# Issue 04 — Offscreen rendering with pixman

## Objective

(PRD Step 2) Render the surface tree into an offscreen BGRA buffer using `smithay::backend::renderer::pixman`. This decouples rendering from both Wayland output state and networking, and produces the exact bytes the wire sends.

## Files

| File | Change |
|---|---|
| `crates/server/src/rendering/mod.rs` | `OffscreenRenderer`: owns `PixmanRenderer`, creates targets, renders the surface map to a buffer |
| `crates/server/src/rendering/target.rs` | `RenderTarget`: `PixmanTarget` wrapper → `readback() -> FrameBuffer` (BGRA, w, h, stride) |
| `crates/server/src/state.rs` | Surface map gains position/layout (trivial tiling: stack surfaces diagonally 0,0 / 20,20 / 40,40… for M1) and damage tracking per surface |
| `crates/server/src/main.rs` | `--snapshot <path>` flag: render once after first client frame, write PNG, exit (PRD's "save framebuffer as PNG" test) |
| `crates/server/tests/render.rs` | Pixel-accuracy tests |
| `crates/server/examples/snapshot.rs` | Example: run server, wait for client, dump PNG |

## Implementation notes

- Smithay 0.7 pixman API: `PixmanRenderer::new()`, `renderer.render(|renderer| { let target = renderer.create_target(w, h, Format::Argb8888)?; ... })` or direct `render_to_target`-style calls — check `docs.rs/smithay/0.7.0` for the exact `PixmanRenderer` surface; the key types are `PixmanRenderer`, `PixmanTarget`, `PixmanTexture`, `PixmanError`. Import textures from client wl_shm buffers via the renderer's `ImportAll`/`ImportMem` capability for shm buffers.
- **BGRA byte order**: pixman Argb8888 on little-endian is BGRA in memory — matches GDI exactly. Assert this in a unit test (render a known pixel 0xAABBGGRR, check bytes) rather than trusting docs.
- Stride: pixman may pad rows; carry the real stride into `FrameBuffer` and the wire `FrameHeader`. Never assume `stride == width * 4`.
- Render pass: clear to opaque black, `render_texture` each tracked surface at its layout position, read back the target's mapped memory.
- Damage: track per-surface damage (accumulate `smithay::wayland::compositor` damage from commits) so issue 05 can fill `FrameHeader`'s damage rects — for M1 the header carries full-frame damage; per-rect tracking lands in issue 05 only if trivial.
- Render trigger: a calloop idle/timer at ~60Hz while any client is attached (frame pacing for M1; real frame-callback pacing is M3 polish).

## Steps

1. `OffscreenRenderer::new(w, h)`; `render(&State) -> FrameBuffer` mapping tracked surfaces → pixman textures → target readback.
2. BGRA byte-order + stride assertion tests.
3. `--snapshot` flag path (PNG via `image`, note `image` expects RGBA — swap R/B channels when writing the debug PNG only; the wire stays BGRA).
4. Integration tests with the issue-03 test harness.
5. Update `lat.md/architecture.md` rendering section + test specs.

## Verification

- Test `renders_client_pattern`: client commits a 64x64 surface where pixel (x,y) = distinctive function (e.g., `0xFF000080 | (x<<8) | y`); after a render tick, readback at layout origin matches the pattern exactly for sampled pixels (corners + center) — full-buffer compare if the harness allows.
- Test `handles_surface_resize`: client re-submits 128x128; next readback is 128x128 region correct.
- Test `bgra_byte_order`: single-pixel render proves memory layout `[B, G, R, A]`.
- Test `stride_reported`: create target with width chosen to trigger padding (if pixman pads — otherwise assert stride == w*4 and document it).
- Manual: `wayland-remote-server --snapshot out.png` + test client → open PNG, see the pattern (PRD Step 2's "If PNG shows a window → success").
- Clippy/fmt clean; `lat check` green.

# 09 — EGL/dmabuf Import Path (probe + software fallback)

## Objective

Support EGL/dmabuf clients (`weston-simple-egl`, GL apps) by advertising a
`zwp_linux_dmabuf` global and importing their dmabuf buffers through a GL
(EGL) renderer. Probe for a usable DRM render node at startup: if one works,
render with a `GlesRenderer` and advertise dmabuf; otherwise fall back to the
existing pixman/`wl_shm` path. This makes the server run on any box that can
run Wayland (hw GPU *or* a virtual GPU like virtio-gpu driven by llvmpipe) —
the GL code is hw/software agnostic.

Design basis: `.agents/skills/egl-dmabuf-feasibility/SKILL.md` (issue 08,
section "GlesRenderer trait parity + EGL/GBM/dmabuf constructor chain").
**Confirmed:** `GlesRenderer` satisfies the same trait set as `PixmanRenderer`
(`Renderer + ImportAll + Offscreen + Bind + ExportMem`); `ExportMem`
(`copy_framebuffer`/`map_texture`) works for Gles via PBO readback, so the
existing readback code is reused verbatim.

## Files

| File | Change |
|------|--------|
| `crates/server/Cargo.toml` | Add smithay features `renderer_gl` + `backend_gbm` (keep `wayland_frontend`, `renderer_pixman`). **Do NOT add** `use_system_lib`. |
| `crates/server/src/rendering/egl.rs` (new) | `probe()` → `Option<GlesSetup { renderer: GlesRenderer, main_device: dev_t, formats: Vec<Format> }`. Glob `/dev/dri/renderD*`; per node: `File::open` → `gbm::Device::new(fd)` → `EGLDisplay::new(gbm)` → capture `dmabuf_render_formats()` + `DrmNode::from_path(...).dev_id()` → `EGLContext::new` → `GlesRenderer::new`. First success wins; log each failure. |
| `crates/server/src/rendering/mod.rs` | `OffscreenRenderer<R: Renderer + ImportAll + Offscreen + Bind + ExportMem>` (generalize the 4 explicit `PixmanTexture`/`PixmanRenderer` refs to `R`/`R::TextureId`). Keep `new()` (pixman) + add `new_gles(...)`. Add `Offscreen` enum `{ Software(OffscreenRenderer<PixmanRenderer>), Gl(OffscreenRenderer<GlesRenderer>) }` with delegating `render`/`render_surface`/`render_window_surface`. |
| `crates/server/src/state.rs` | `renderer: Option<Offscreen>` field. Add `dmabuf_state: DmabufState` (always created; global registered only on Gles). Implement `DmaBufHandler` (+ `BufferHandler` if not already) and `delegate_dmabuf!(State)`. `State::new` takes `Option<(dev_t, Vec<Format>)>` and, if `Some`, builds `DmabufFeedbackBuilder::new(main_device, formats).build()` + `dmabuf_state.create_global_with_default_feedback::<State>(&dh, &fb)`. |
| `crates/server/src/lib.rs` | In `run()`: call `egl::probe()`; on success pass `(main_device, formats)` to `State::new` and build `Offscreen::Gl(OffscreenRenderer::new_gles(renderer, w, h))`; on failure `State::new(None)` + `Offscreen::Software(OffscreenRenderer::new(w, h)?)`. Render call sites use the `Offscreen` enum (same method names). |

## Subtasks

### 09a — Probe module + features (S)
Add Cargo features + build deps (`libgbm-dev`, `libdrm-dev` on gary-agents).
New `rendering/egl.rs` with `probe()`. Compiles; on gary-agents `probe()`
returns `Some`.

### 09b — Generic renderer + Offscreen enum + wire probe (S)
Generalize `OffscreenRenderer<R>`; add `Offscreen` enum; `run()` probes and
builds the matching variant. Both variants constructible; pixman path unchanged
behavior.

### 09c — dmabuf global on State (S)
`DmaBufHandler` + `delegate_dmabuf!` + conditional global registration (only on
the Gles path). Thread `(main_device, formats)` from `run()` into `State::new`.
`zwp_linux_dmabuf` advertised only when a render node is available.

## Verification

- `cargo clippy --workspace --tests -- -D warnings` clean on gary-agents.
- `cargo test --workspace` green (existing 18 suites + no regression; the
  pixman path is exercised by the existing subsurface/render tests).
- Live on gary-agents: `weston-simple-egl` maps and renders (drive viewer
  captures non-black frames); the registry shows `zwp_linux_dmabuf`.
- Fallback: with no usable render node the server still starts with the pixman
  path and does NOT advertise `zwp_linux_dmabuf` (log line present).
- `lat check` green; `lat.md` rendering section updated for the EGL/dmabuf
  path + probe/fallback.

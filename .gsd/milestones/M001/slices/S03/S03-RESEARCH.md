# Phase 3: Headless Rendering - Research

**Researched:** 2026-03-10
**Domain:** Headless/offscreen rendering with Smithay PixmanRenderer
**Confidence:** HIGH

## Summary

Phase 3 requires implementing headless/offscreen rendering using Smithay's PixmanRenderer. Based on research of Smithay 0.7.0, the architecture involves:

1. **PixmanRenderer**: Software renderer using pixman library for CPU-based rendering without GPU/display requirements
2. **Offscreen Trait**: Create memory-based framebuffers via `create_buffer()` for headless operation
3. **ExportMem Trait**: Extract RGBA pixel data from rendered framebuffers using `copy_framebuffer()` + `map_texture()`
4. **Buffer Lifecycle**: Handle attach → render → release sequence via CompositorHandler hooks
5. **Frame Callbacks**: Critical to respond to `wl_surface.frame` to prevent client freezing

**Primary recommendation:** Use PixmanRenderer with `Offscreen<Image>` trait for creating memory-backed framebuffers, render surfaces to these buffers, then use `ExportMem` to extract RGBA data for streaming.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| REND-01 | Compositor uses headless/offscreen rendering (PixmanRenderer) | `PixmanRenderer::new()` creates software renderer; `Offscreen::create_buffer()` makes memory framebuffers; no display/GPU required |
| REND-02 | Surface content is rendered to an offscreen buffer/framebuffer | `Renderer::render()` draws to `PixmanTarget`; `render_texture_from_to()` composites surface textures; `PixmanFrame` handles rendering ops |
| REND-03 | Framebuffer can be read back as RGBA pixel data | `ExportMem::copy_framebuffer()` copies to `PixmanMapping`; `map_texture()` returns `&[u8]` RGBA slice; format is `DrmFourcc::Abgr8888` |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| smithay | 0.7.0 | Wayland compositor framework | Provides PixmanRenderer, Offscreen trait, ExportMem trait |
| pixman | 0.2.1 (crate) | Low-level pixel manipulation | Smithay dependency for software rendering operations |
| wayland-server | 0.31.9 | Wayland protocol server | Smithay dependency; handles wl_surface and wl_buffer |
| calloop | 0.14.0 | Event loop framework | Required for frame callback timing integration |
| drm-fourcc | 2.2.0 | Pixel format definitions | Fourcc codes for buffer formats (Abgr8888, Xrgb8888) |

### Smithay Features Required
For Phase 3 (headless rendering):
- `wayland_frontend` (required for surface/buffer handling)
- `renderer_pixman` (enables PixmanRenderer)

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| PixmanRenderer | GlesRenderer | Gles requires GPU/DRM; Pixman is pure CPU, truly headless |
| PixmanRenderer | GlowRenderer | Glow wraps Gles; same GPU requirements |
| Offscreen buffers | GBM buffers | GBM requires DRM; offscreen pixman Image is fully headless |
| RGBA readback | GPU DMA-BUF export | DMA-BUF requires GPU; ExportMem works with CPU buffers |

**Installation:**
```toml
[dependencies]
smithay = { version = "0.7.0", features = ["wayland_frontend", "renderer_pixman"] }
pixman = { version = "0.2.1" }  # Usually pulled via smithay
```

## Architecture Patterns

### Recommended Project Structure
```
crates/server/src/
├── main.rs              # Entry point, event loop setup
├── state.rs             # CompositorState with renderer field
├── handlers/
│   ├── compositor.rs    # CompositorHandler - render on commit
│   ├── frame_callback.rs # Frame callback timing management
│   └── buffer_release.rs # Buffer lifecycle tracking
├── rendering/
│   ├── offscreen.rs     # Offscreen buffer management
│   ├── surface_renderer.rs # Surface → texture → framebuffer
│   └── pixel_export.rs  # RGBA extraction from framebuffers
└── surface.rs           # Per-surface state (buffer, frame callbacks)
```

### Pattern 1: Offscreen Buffer Creation
**What:** Create memory-backed framebuffer for headless rendering
**When to use:** Every surface needs its own render target
**Key components:**
- `PixmanRenderer` implements `Offscreen<Image<'static, 'static>>`
- `create_buffer(format, size)` returns `pixman::Image` for rendering
- Must use format `DrmFourcc::Abgr8888` or `DrmFourcc::Xrgb8888`
- Buffer lifetime tied to surface (resize = recreate)

**Example:**
```rust
// Source: https://docs.rs/smithay/0.7.0/smithay/backend/renderer/trait.Offscreen.html
use smithay::backend::renderer::{
    pixman::PixmanRenderer,
    Offscreen,
    allocator::Fourcc,
};
use smithay::utils::{Size, Buffer};

fn create_offscreen_buffer(
    renderer: &mut PixmanRenderer,
    width: i32,
    height: i32,
) -> Result<pixman::Image<'static, 'static>, PixmanError> {
    let size = Size::from((width, height));
    // Create headless memory buffer in RGBA format
    let buffer = renderer.create_buffer(Fourcc::Abgr8888, size)?;
    Ok(buffer)
}
```

### Pattern 2: Surface Rendering to Offscreen Buffer
**What:** Render Wayland surface content to offscreen framebuffer
**When to use:** On surface commit, when buffer is attached
**Key components:**
- Import buffer using `ImportMemWl::import_shm_buffer()`
- Bind offscreen target using `Bind::bind()`
- `Renderer::render()` returns `PixmanFrame` for drawing
- `Frame::render_texture_from_to()` composites surface texture

### Pattern 3: RGBA Pixel Extraction
**What:** Read back framebuffer contents as RGBA bytes
**When to use:** After rendering, before streaming
**Key components:**
- `ExportMem::copy_framebuffer()` copies to mapping
- `map_texture()` returns `&[u8]` slice of pixel data
- Format is Abgr8888 (ARGB in little-endian)
- Must hold mapping reference until transmission complete

### Pattern 4: Buffer Lifecycle Management
**What:** Handle attach → render → release sequence
**When to use:** Track when buffer is safe to release
**Key components:**
- Use `add_post_commit_hook()` for post-render operations
- Buffer release happens AFTER RGBA extraction AND transmission
- Track buffer reference per-surface
- Release via `wl_buffer.destroy()` when done

### Pattern 5: Frame Callback Timing
**What:** Respond to wl_surface.frame to drive client rendering
**When to use:** After each successful render, throttle to network
**Key components:**
- Clients request frame callbacks via `wl_surface.frame()`
- Callbacks queued in SurfaceAttributes
- Must call `callback.done(time)` to signal ready for next frame
- Throttle rate to network capacity to prevent buffer bloat

### Anti-Patterns to Avoid
- **Releasing buffer before RGBA extraction**: Client will corrupt next frame
- **Not sending frame callbacks**: Applications freeze waiting for render signal
- **Creating framebuffer on every commit**: Expensive; reuse or resize only when needed
- **Ignoring damage regions**: Full-frame rendering wastes CPU; use damage tracking
- **Blocking on render**: Do render + extract in separate thread to avoid event loop blocking

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Offscreen pixel buffer | Manual Vec<u8> allocation | `PixmanRenderer::create_buffer()` | Handles row alignment, format conversion, pixman integration |
| Buffer format conversion | Manual pixel manipulation | `ExportMem` with `Fourcc::Abgr8888` | Pixman optimizes format conversion; handles endianness |
| Surface texture import | Manual wl_buffer parsing | `ImportMemWl::import_shm_buffer()` | Handles SHM protocol, stride, offset, format negotiation |
| Rendering | Custom draw calls | `Frame::render_texture_from_to()` | Proper damage tracking, blending, transformations |
| Frame callback tracking | Manual HashMap | `SurfaceAttributes::frame_callbacks` | Smithay manages callback queue per-surface |
| Buffer release timing | Manual reference counting | `add_post_commit_hook()` | Properly sequenced after commit processing |

**Key insight:** Smithay's renderer abstractions handle pixel format negotiation, coordinate transformations, damage tracking, and buffer lifecycle correctly. Custom implementations would break protocol compliance and introduce rendering bugs.

## Common Pitfalls

### Pitfall 1: Buffer Release Too Early
**What goes wrong:** Visual artifacts, client crashes, or frozen frames
**Why it happens:** Buffer released before compositor finishes reading pixel data
**How to avoid:**
- Keep buffer reference until `map_texture()` completes
- Release in post-commit hook, not during commit handler
- For streaming: release only after TCP transmission confirms
- **Critical:** Never call `buffer.release()` before RGBA extraction

### Pitfall 2: Missing Frame Callbacks
**What goes wrong:** Applications freeze or render at incorrect rate
**Why it happens:** `wl_surface.frame` callbacks not sent to drive client rendering loop
**How to avoid:**
- Always send frame callbacks after rendering surface
- Use `attrs.frame_callbacks` from `SurfaceAttributes`
- Call `callback.done(time)` with millisecond timestamp
- Throttle to target framerate (30fps = 33ms interval)
- **Warning sign:** Client appears frozen but process is running

### Pitfall 3: Pixman Format Confusion
**What goes wrong:** Wrong colors, BGRA vs RGBA mismatch, alpha issues
**Why it happens:** DrmFourcc format names don't match memory layout
**How to avoid:**
- Use `Fourcc::Abgr8888` for RGBA in memory (little-endian ARGB)
- Verify format: Abgr8888 = [B, G, R, A] in memory on little-endian
- Test with colored window to verify channel order
- Be consistent across renderer, export, and streaming

### Pitfall 4: Offscreen Buffer Size Mismatch
**What goes wrong:** Rendering clipped or scaled incorrectly
**Why it happens:** Buffer size doesn't match surface geometry
**How to avoid:**
- Query surface size from `SurfaceAttributes`
- Recreate buffer when surface is resized
- Ensure buffer size matches texture dimensions
- Check `width` and `height` from texture import

### Pitfall 5: Frame Lifecycle Errors
**What goes wrong:** Panic on drop, resource leaks, undefined framebuffer state
**Why it happens:** Frame not properly finished or leaked
**How to avoid:**
- Always call `frame.finish()` before dropping
- Don't store Frame across await points
- Handle errors from `finish()` - may indicate render failure
- Use `Drop` impl on PixmanFrame, but explicit finish is safer

### Pitfall 6: PixmanRenderer Not Send/Sync
**What goes wrong:** Can't share renderer across threads
**Why it happens:** Pixman uses thread-local state
**How to avoid:**
- Create renderer in main thread only
- Use channel to send RGBA data to streaming thread
- Don't try to use renderer in multiple threads
- Serialize render operations on single thread

## Sources

### Primary (HIGH confidence)
- Smithay 0.7.0 PixmanRenderer docs: https://docs.rs/smithay/0.7.0/smithay/backend/renderer/pixman/index.html - API reference
- Smithay Offscreen trait: https://docs.rs/smithay/0.7.0/smithay/backend/renderer/trait.Offscreen.html - Headless buffer creation
- Smithay ExportMem trait: https://docs.rs/smithay/0.7.0/smithay/backend/renderer/trait.ExportMem.html - RGBA readback
- Smithay Frame trait: https://docs.rs/smithay/0.7.0/smithay/backend/renderer/trait.Frame.html - Rendering operations
- Smithay Renderer trait: https://docs.rs/smithay/0.7.0/smithay/backend/renderer/trait.Renderer.html - Core renderer API

### Secondary (MEDIUM confidence)
- Smithay Compositor docs: https://docs.rs/smithay/0.7.0/smithay/wayland/compositor/index.html - Buffer lifecycle hooks
- Anvil reference implementation: https://github.com/Smithay/smithay/tree/master/anvil - Production patterns

### Tertiary (LOW confidence)
- Pixman library docs: https://pixman.org/ - Low-level pixel operations (Smithay abstracts this)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Smithay 0.7.0 is released, APIs are stable
- Architecture: HIGH - Anvil/smallvil provide working examples, clear trait boundaries
- Pitfalls: MEDIUM - Some from docs verification, others from Phase 2 patterns (buffer release timing)

**Research date:** 2026-03-10
**Valid until:** 2026-09-10 (Pixman rendering is stable; unlikely to change significantly)

---

**Next Steps for Planner:**
1. Create PLAN.md with offscreen buffer initialization sequence
2. Priority: PixmanRenderer::new() → create_buffer() → import_shm_buffer() → render() → copy_framebuffer()
3. Critical verification: Buffer release timing (after RGBA extraction)
4. Critical verification: Frame callback handling to prevent client freezes
5. Test with simple Wayland client to verify RGBA output format
---
name: egl-dmabuf-feasibility
description: >-
  Research findings for issue 08: can the pixman/shm-only wayland-remote
  compositor support EGL/dmabuf (GPU) clients like weston-simple-egl?
  Exact gary-agents environment probe, smithay 0.7.0 EGL/dmabuf/GBM API
  surface, and an honest feasibility verdict (incl. why a GPU-less headless
  box cannot import dmabuf in smithay 0.7.0). Read this before scoping any
  GPU/EGL follow-up — do NOT re-research.
---

# EGL / dmabuf Feasibility for wayland-remote (issue 08)

Research-only deliverable. Verdict up front: **the EGL/dmabuf path is feasible
on gary-agents because it actually has NVIDIA GPUs — but a truly GPU-less
headless box CANNOT import dmabuf in smithay 0.7.0** (no wgpu renderer,
surfaceless EGL can't import dmabuf, GBM needs a render node). Recommendation:
**defer implementation now**, keep a scoped follow-up plan for the GPU-backed
case, and gate it on a deployment-hardware decision. Details + exact evidence
below.

## TL;DR

| Question | Answer |
| --- | --- |
| EGL implementation on gary-agents? | Yes. `libEGL.so.1` (+ `libEGL_nvidia.so.0`, `libEGL_mesa.so.0`), `libgbm.so.1`, `libGLESv2.so.2`. `eglinfo`/`mesa-utils` NOT installed (installable, candidate 9.0.0-2build1). |
| Render nodes on gary-agents? | **Yes — 3x NVIDIA GeForce RTX 5060 Ti (GB206)**, `/dev/dri` has `card0..2` + `renderD128/129/130`. Driver 595.84, CUDA 13.2. (Contradicts the "no GPU expected" assumption.) |
| GPUs idle? | No. nvidia-smi shows ~15.3–15.7 GiB of 16 GiB used and 88–92% util on all three (other workloads). |
| `weston-simple-egl` installed? | Yes, `/usr/bin/weston-simple-egl`. |
| Software DRI / Vulkan? | `swrast_dri.so` + `kms_swrast_dri.so` present; Vulkan ICDs `lvp_icd.json` (lavapipe) + `nvidia_icd.json` present; `mesa-vulkan-drivers` 26.0.3 + `libvulkan1` 1.4.341 installed. `vulkaninfo` not installed. |
| smithay 0.7.0 GL/EGL renderer? | `renderer_gl` feature → `GlesRenderer` in `src/backend/renderer/gles/` (gated `#[cfg(feature = "renderer_gl")]`, renderer/mod.rs:27 — **correction**: the earlier note said `renderer_glow`, but that feature adds a different `GlowRenderer`). **There is NO `renderer_egl` and NO `renderer_wgpu` feature.** |
| Can surfaceless EGL import dmabuf? | **Effectively no.** On Mesa 26 a surfaceless software display DOES advertise `EGL_EXT_image_dma_buf_import`, but on gary-agents it cannot create any GL context (all configs → `EGL_BAD_CONFIG`/`EGL_BAD_ATTRIBUTE`), so `get_dmabuf_formats`/`create_image_from_dmabuf`/`GlesRenderer` have nothing to render through. Dead end. See "Software-EGL-over-GBM fallback (llvmpipe)". |
| Does GBM + software (llvmpipe) EGL work on gary-agents? | **No.** Mesa ICD `eglInitialize` FAILS on the NVIDIA render nodes (libgbm loads the NVIDIA GBM backend `nvidia-drm_gbm.so`; Mesa can't attach a software DRI driver). `LIBGL_ALWAYS_SOFTWARE=1` alone is ignored (NVIDIA ICD still used). Only the NVIDIA **hw** GBM path works and advertises dmabuf import. Unverified (not testable here) on a virtio-gpu / generic-`dri`-backend VM. See "Software-EGL-over-GBM fallback (llvmpipe)". |
| Can a GPU-less box do EGL/dmabuf at all? | **No in smithay 0.7.0.** No render node → no GBM → no device-backed EGL display; no wgpu/lavapipe renderer; the EGL client itself also needs a render node to allocate its dmabuf. |
| Current server import limit? | **wl_shm only.** `PixmanRenderer::import_buffer` in `crates/server/src/rendering/mod.rs`; `state.rs` has no `delegate_dmabuf`/`DmabufState`. |

## Environment: what is actually on gary-agents

Probed via `ssh gary-agents`. gary-agents is Ubuntu 25.10 "Resolute" (mirror
`gtlib.gatech.edu`), arch amd64. Key outputs (abbreviated):

```
$ eglinfo -B ; eglinfo -query
eglinfo: command not found            # mesa-utils not installed

$ ldconfig -p | grep -iE "egl|gbm|glesv2|libGL"   (relevant subset)
libEGL.so.1            => /usr/lib/x86_64-linux-gnu/libEGL.so.1
libEGL_nvidia.so.0     => .../libEGL_nvidia.so.0
libEGL_mesa.so.0       => .../libEGL_mesa.so.0
libgbm.so.1            => .../libgbm.so.1
libGLESv2.so.2 / libGLESv2_nvidia.so.2 / libGLESv1_CM.so.1 / _nvidia
libGL.so.1 / libGLX.so.0 / libGLX_nvidia.so.0 / libGLX_mesa.so.0 / libGLdispatch.so.0
libnvidia-eglcore.so.595.84 / libnvidia-egl-gbm.so.1 / libnvidia-egl-wayland.so.1 (+ xlib/xcb/wayland2)
libwayland-egl.so.1

$ ls -la /dev/dri/
card0  card1  card2
renderD128  renderD129  renderD130
by-path: pci-0000:05:00.0-render->renderD128
         pci-0000:06:00.0-render->renderD129
         pci-0000:07:00.0-render->renderD130

$ for c in /sys/class/drm/card[0-2]; do grep DRIVER $c/device/uevent; done
DRIVER=nvidia   (x3)      # PCI_ID=10DE:2D04 (all three)

$ lspci | grep -iE "vga|3d|display"
05:00.0 VGA ...: NVIDIA GB206 [GeForce RTX 5060 Ti] (rev a1)
06:00.0 VGA ...: NVIDIA GB206 [GeForce RTX 5060 Ti] (rev a1)
07:00.0 VGA ...: NVIDIA GB206 [GeForce RTX 5060 Ti] (rev a1)

$ nvidia-smi  (driver 595.84, CUDA 13.2)
GPU 0 05:00.0  15768MiB/16311MiB  88% util   Disp.A=On
GPU 1 06:00.0  15318MiB/16311MiB  91% util   Disp.A=Off
GPU 2 07:00.0  15548MiB/16311MiB  92% util   Disp.A=Off

$ which weston-simple-egl
/usr/bin/weston-simple-egl

$ dpkg -l | grep -iE "mesa|libegl|libgles|libgbm"   (version 26.0.3-1ubuntu1 unless noted)
libegl1 1.7.0-3   libegl-mesa0 26.0.3   libgbm1 26.0.3
libgl1-mesa-dri 26.0.3   libglx-mesa0 26.0.3   mesa-libgallium 26.0.3
libgles1 1.7.0-3   libgles2 1.7.0-3   mesa-vulkan-drivers 26.0.3

$ apt-cache policy libvulkan1 mesa-vulkan-drivers
libvulkan1: Installed: 1.4.341.0-1
mesa-vulkan-drivers: Installed: 26.0.3-1ubuntu1

$ apt-cache policy mesa-utils
mesa-utils: Installed: (none)  Candidate: 9.0.0-2build1   (universe)

$ ls /usr/lib/x86_64-linux-gnu/dri/ | grep -iE "swrast|lvp"
kms_swrast_dri.so
swrast_dri.so

$ ls /usr/share/vulkan/icd.d/
lvp_icd.json  nvidia_icd.json  intel_icd.json  nouveau_icd.json  radeon_icd.json
asahi_icd.json  intel_hasvk_icd.json  virtio_icd.json  gfxstream_vk_icd.json
$ which vulkaninfo     # not installed

$ dpkg -l | grep -iE "libgbm-dev|libdrm-dev|libegl1-mesa-dev|pkg-config"
pkg-config 2.5.1-4        # gbm/drm/egl -dev NOT installed -> must apt-get install to build
```

Implication: gary-agents is effectively a **GPU dev box**, not a headless
no-GPU VM. EGL/GBM/dmabuf would work here against the NVIDIA render nodes
(contending with the other 88–92% workloads). The "headless, no GPU" premise
in issue 08 is not true of this machine.

## smithay 0.7.0 API surface

Source: `/home/gary/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/smithay-0.7.0/`.
Layout note: `dmabuf` and `gbm` allocators are single files; there is no
`renderer/egl/` or `renderer/wgpu/` directory.

### Feature flags (Cargo.toml `[features]`)

```
renderer_gl   = ["gl_generator", "backend_egl"]   # <-- the GlesRenderer (can import EGL/dmabuf)
renderer_glow = ["renderer_gl", "glow"]           # different GlowRenderer (renderer/glow.rs), not needed
renderer_pixman = ["pixman"]
renderer_multi = ["backend_drm", "aliasable"]
renderer_test  = []

backend_egl   = ["gl_generator", "libloading"]   # EGL loaded at runtime via libloading (no -dev needed to build)
backend_gbm   = ["gbm", "cc", "pkg-config", "backend_drm"]
backend_drm   = ["drm", "drm-ffi"]
backend_vulkan = ["ash", "scopeguard"]           # Vulkan ALLOCATOR only (src/backend/allocator/vulkan/), NOT a renderer
```

There is **no `renderer_egl`** feature and **no `renderer_wgpu`** feature.
`grep -r wgpu src/` → empty. The GL path is `renderer_gl` (GlesRenderer). To
get an EGL-capable renderer that imports dmabuf, enable **`renderer_gl` +
`backend_gbm`** (the latter pulls `gbm` + `backend_drm`).

Current server config (`Cargo.toml:16`): `smithay = { version = "0.7.0",
default-features = false, features = ["wayland_frontend", "renderer_pixman"] }`.

### EGL backend — `src/backend/egl/` (gated `backend_egl`)

- `display.rs`
  - `EGLDisplay` fields include `dmabuf_import_formats: FormatSet`,
    `dmabuf_render_formats: FormatSet`, `has_fences`, `supports_native_fences`.
  - `unsafe fn EGLDisplay::new(native: impl EGLNativeDisplay) -> Result<EGLDisplay, Error>`.
    `select_platform_display` tries each `native.supported_platforms()` entry via
    `eglGetPlatformDisplayEXT`, skipping platforms whose required extension is missing.
  - `get_dmabuf_formats` (line ~868): **if the display lacks `EGL_EXT_image_dma_buf_import`
    it returns `(FormatSet::default(), FormatSet::default())` — i.e. EMPTY.** Otherwise
    queries `eglQueryDmaBufFormatsEXT` + `eglQueryDmaBufModifiersEXT`.
  - `EGLDisplay::create_image_from_dmabuf(&Dmabuf) -> Result<EGLImage, Error>` (line ~727):
    requires `EGL_KHR_image_base` OR `EGL_EXT_image_dma_buf_import`.
  - `EGLBufferReader` (line ~1009): created by `bind_wl_display`, used to map wl_drm
    dmabuf fd→`EGLImage`.
- `context.rs`
  - `EGLContext`; `EGLContext::new(display)`, `new_with_config`, `new_shared`,
    `new_shared_with_config` (shared context needed for multi-context texture sharing).
    Context creation references `EGL_KHR_surfaceless_context` (line ~228).
- `surface.rs`: `EGLSurface`.
- `device.rs`
  - `EGLDevice`; `EGLDevice::enumerate()` (needs `EGL_EXT_device_base`/`_enumeration`/
    `_query`); `EGLDevice::device_for_display(&EGLDisplay)`; `try_get_render_node()`.
- `native.rs` — `EGLNativeDisplay` impls (what you pass to `EGLDisplay::new`):
  - `impl EGLNativeDisplay for GbmDevice<A>` (line ~147, gated `backend_gbm`):
    tries `EGL_KHR_platform_gbm` then `EGL_MESA_platform_gbm`.
  - `impl EGLNativeDisplay for EGLDevice` (line ~236): `EGL_EXT_platform_device`.
    (Comment: "EGLDisplays based on EGLDevices do not support normal windowed surfaces.")
  - `X11DefaultDisplay` (line ~223).
  - `EGLSurfacelessDisplay` (line ~260): `EGL_MESA_platform_surfaceless`, native display
    = `DEFAULT_DISPLAY`. **No device required — this is the only path usable without
    /dev/dri.**

### GlesRenderer — `src/backend/renderer/gles/mod.rs` (gated `renderer_gl`)

```
pub struct GlesRenderer {            // line 280
    gl: ffi::Gles2,
    egl: EGLContext,                 // line 287
    egl_reader: Option<EGLBufferReader>,
    dmabuf_cache: HashMap<WeakDmabuf, GlesTexture>,   // line 303
    ...
}
unsafe fn GlesRenderer::new(context: EGLContext) -> Result<GlesRenderer, GlesError>   // line 468
unsafe fn GlesRenderer::supported_capabilities(context: &EGLContext) -> Result<Vec<Capability>, GlesError>  // line 403
pub enum Capability { ... ExportFence /* needs GL_OES_EGL_sync */ }  // line 262
```

Import impls (all on `GlesRenderer`):
- `import_shm_buffer` (line ~765) — SHM.
- `import_memory` (line ~936).
- `impl ImportEgl for GlesRenderer` (line ~1097, gated `use_system_lib`):
  - `bind_wl_display(&mut self, display) -> Result<(), Error>` → `self.egl_reader =
    Some(self.egl.display().bind_wl_display(display)?)` (line ~1102). **Only needed for
    wl_drm/EGL-buffer import — NOT for zwp_linux_dmabuf.**
  - `import_egl(&mut self, surface: &mut EGLSurface, format: &EGLFormat, ...)` (line ~1097).
  - `import_egl_buffer` (line ~1116) — EGLSurface-backed buffer (requires `egl_reader`).
  - `egl_reader()`.
- `import_dmabuf(&mut self, buffer: &Dmabuf, ...)` (line ~1169):
  ```
  self.existing_dmabuf_texture(buffer)?.map(Ok).unwrap_or_else(|| {
      let is_external = !self.egl.dmabuf_render_formats().contains(&buffer.format());
      let image = self.egl.display().create_image_from_dmabuf(buffer)?;  // egl_reader NOT involved
      self.import_egl_image(image, is_external, None)?
  })
  ```
  **Requires the `GL_OES_EGL_image` GL extension and a display that can
  `create_image_from_dmabuf`** (i.e. a device-backed EGL display advertising
  dmabuf import, not surfaceless). `egl_reader` is NOT required — that is only
  for the wl_drm/EGL-buffer path (`import_egl_buffer` @1116).

`ImportAll` (so `renderer.import_buffer(&WlBuffer)` works) is implemented via
`Renderer + ImportMemWl + ImportEgl + ImportDmaWl` (`renderer/mod.rs` line ~683).

### Import traits — `src/backend/renderer/mod.rs`

- `pub trait ImportMemWl: ImportMem` (line ~452): `import_shm_buffer`.
- `pub trait ImportDmaWl: ImportDma` (line ~591): `import_dmabuf`.
- `pub trait ImportEgl: Renderer` (line ~540): `import_egl`, `bind_wl_display`,
  `import_egl_buffer`, `egl_reader`.
- `pub trait ImportAll: Renderer` (line ~650): blanket impls at ~683 and ~704.

### GBM — `src/backend/allocator/gbm.rs` (gated `backend_gbm`)

- `pub use gbm::{BufferObjectFlags as GbmBufferFlags, Device as GbmDevice}` (line 15).
  `GbmDevice` is the `gbm` crate's `Device` — it opens a **DRM render node** (needs
  `/dev/dri/renderD*` or a card). `GbmDevice::new(path: impl AsFd)` / `gbm::Device::main()`
  both fail without a DRM device. There is **no swrast fallback for GBM**.
- `GbmAllocator::new(device: GbmDevice<A>, flags: GbmBufferFlags)` (line 177) — allocates
  GBM buffers for the compositor's own swapchain (not needed for import-only).
- `Dmabuf::import_to(gbm: &GbmDevice<A>, usage: GbmBufferFlags) -> io::Result<GbmBuffer>`
  (line ~349) — import a dmabuf into a GBM BO.

### Wayland dmabuf — `src/wayland/dmabuf/` (mod.rs + dispatch.rs)

> **CORRECTED 2026-08-21** (see "GlesRenderer trait parity + EGL/GBM/dmabuf
> constructor chain (verified)" below): there is **no public
> `DmabufGlobalState::new`** in 0.7.0 — that struct is private (mod.rs:568).
> The line ~315 signature is actually `DmabufFeedbackBuilder::new`, and
> `gbm::Device::main()` does not exist in gbm 0.18 (only `Device::new(fd)`).
> Globals are created via `DmabufState::create_global*`.

- `DmabufFeedbackBuilder::new(main_device: libc::dev_t, formats: impl IntoIterator<Item = Format>)`
  (line 315). `main_device` is the DRM dev_t of your render node.
- `DmabufState` (line ~580), `DmabufState::new()` (line 588). Holds the
  `zwp_linux_dmabuf` objects; `create_global*` registers the global on the
  `DisplayHandle` internally (mod.rs:733).
- `pub trait DmabufHandler: BufferHandler` (line ~992):
  - `fn dmabuf_state(&mut self) -> &mut DmabufState;`
  - `fn dmabuf_imported(&mut self, global: &DmabufGlobal, dmabuf: Dmabuf, notifier: ImportNotifier);`
    → import `dmabuf` into the renderer here (via `renderer.import_buffer`/`import_dmabuf`)
    and reply success through `notifier`.
  - `fn new_surface_feedback(&mut self, _surface: &WlSurface, _global: &DmabufGlobal)
    -> Option<DmabufFeedback> { None }` (optional).
- `macro_rules! delegate_dmabuf!($ty)` (line ~1037): delegates `ZwpLinuxDmabufV1`,
  `ZwpLinuxBufferParamsV1`, `WlBuffer`, `ZwpLinuxDmabufFeedbackv1` to `DmabufState`.
- `pub fn get_dmabuf(buffer: &WlBuffer) -> Result<&Dmabuf, UnmanagedResource>`.
- The `Dmabuf` type itself lives in `src/backend/allocator/dmabuf.rs` (single file).

## GlesRenderer trait parity + EGL/GBM/dmabuf constructor chain (verified)

Verified 2026-08-21 against the smithay 0.7.0 source (line numbers below are
from that tree). **Bottom line: `GlesRenderer` implements the exact same trait
set as `PixmanRenderer`, including `ExportMem` (copy_framebuffer + map_texture
via PBO/ReadPixels + MapBufferRange), so `OffscreenRenderer` CAN be made
generic and the existing render/readback code works unchanged for both.**

### CRUX: trait parity table

| Trait | `PixmanRenderer` (renderer/pixman/mod.rs) | `GlesRenderer` (renderer/gles/mod.rs) |
| --- | --- | --- |
| `RendererSuper` assoc. types | @810: `Error=PixmanError`, `TextureId=PixmanTexture`, `Framebuffer<'b>=PixmanTarget<'b>`, `Frame=PixmanFrame` | @2007: `Error=GlesError`, `TextureId=GlesTexture`, `Framebuffer<'b>=GlesTarget<'b>`, `Frame=GlesFrame` |
| `Renderer` | `impl` @820 | `impl` @2017 |
| `ImportMem` | @888 | @933 |
| `ImportMemWl` (cfg `wayland_frontend`) | @1120 | @762 |
| `ImportDma` | @1167 (real: CPU-maps dmabuf via `DmabufMappingMode::READ`; linear formats only, `dmabuf_formats` @1183) | @1166 (real: `create_image_from_dmabuf` → EGLImage → texture; `dmabuf_formats` @1208 = display's `dmabuf_texture_formats`) |
| `ImportDmaWl` (cfg `wayland_frontend`) | empty impl @1199 | empty impl @1218 (default `import_dma_buffer` dispatches Shm/Dma via `get_dmabuf`) |
| `ImportEgl` (cfg `wayland_frontend`+`backend_egl`+`use_system_lib`) | @1095 — **STUB**: `bind_wl_display` returns `Err(NoEGLDisplayBound)` | @1097 — real: `bind_wl_display` @1098 sets `egl_reader` |
| `ImportAll` (blanket, renderer/mod.rs) | via blanket @704 (`Renderer+ImportMemWl+ImportDmaWl`) or @683 (`+ImportEgl`, needs `use_system_lib`) | same blanket impls — **no direct impl needed** |
| `Offscreen<T>` | `Offscreen<Image<'static,'static>>` @1246: `create_buffer(&mut self, Fourcc, Size<i32,BufferCoord>) -> Result<Image<'static,'static>, PixmanError>` | `Offscreen<GlesTexture>` @1559 (also `Offscreen<GlesRenderbuffer>` @1599): `create_buffer(...) -> Result<GlesTexture, GlesError>` |
| `Bind<T>` | `Bind<Image<'static,'static>>` @1261, `Bind<Dmabuf>` @1201 | `Bind<EGLSurface>` @1425, `Bind<Dmabuf>` @1431, `Bind<GlesTexture>` @1511, `Bind<GlesRenderbuffer>` @1517 |
| `ExportMem` | @1003: `TextureMapping=PixmanMapping`; `copy_framebuffer` @1007; `map_texture` @1077 (returns `stride*height` bytes) | @1268: `TextureMapping=GlesMapping`; **`copy_framebuffer` @1273** (PBO `BufferData`+`ReadPixels`, @1284-1311); **`map_texture` @1390** (`MapBufferRange` on the PBO, @1401-1421; returns `w*h*4` bytes @1399) |

So `R: Renderer + ImportAll + Offscreen<P> + Bind<P> + ExportMem` (where
`Offscreen<P>` already implies `Renderer + Bind<P>`) is satisfied by **both**.

Gles readback specifics (why the readback path is safe on GL):
- `Offscreen<GlesTexture>::create_buffer` (gles/mod.rs:1559-1596) builds the
  texture with `GlesTexture::from_raw(self, Some(internal), !has_alpha, ...)` —
  the format IS stored, and `GlesTargetInternal::Texture::format()` returns
  `Some` (gles/mod.rs:210). `copy_framebuffer` requires
  `target.0.format()` (@1281) — satisfied.
- `fourcc_to_gl_formats(Fourcc::Argb8888) = (BGRA_EXT, BGRA_EXT, UNSIGNED_BYTE)`
  (gles/format.rs:21) — `create_buffer(Fourcc::Argb8888)` is accepted (BGRA_EXT
  is in the allowed set, gles/mod.rs:1570-1574) and `copy_framebuffer(...,
  Fourcc::Argb8888)` maps back to BGRA bytes — same wire format as pixman.
- `map_texture` returns exactly `w*h*4` bytes (gles/mod.rs:1399) with no row
  padding; pixman returns `stride*height` with `stride==w*4` for Argb8888. The
  current `pixels.to_vec()` + `stride: width*4` in rendering/mod.rs holds for both.
- `frame.finish()?.wait()` is renderer-agnostic (`SyncPoint::wait`); Gles
  `Renderer::wait` handles `EGLFence` (gles/mod.rs:2107+).
- `GlesRenderer` is **!Send** (`PhantomData<*mut ()>`, gles/mod.rs:314) — it must
  stay on the compositor thread, which is already true (render requests are
  consumed on the calloop loop thread).

### Constructor chain (verbatim signatures)

Render-node selection:

```rust
// EGLDevice — smithay::backend::egl::device (gated backend_egl)
pub fn enumerate() -> Result<impl Iterator<Item = EGLDevice>, Error>        // egl/device.rs:27
pub fn device_for_display(display: &EGLDisplay) -> Result<EGLDevice, Error> // egl/device.rs:90
pub fn drm_device_path(&self) -> Result<PathBuf, Error>                     // egl/device.rs:140
pub fn render_device_path(&self) -> Result<PathBuf, Error>                  // egl/device.rs:173
pub fn try_get_render_node(&self) -> Result<Option<DrmNode>, Error>         // egl/device.rs:214 (gated backend_drm)
pub fn is_software(&self) -> bool                                           // egl/device.rs:240
// NOTE: on gary-agents the glvnd dispatch returns null for per-device string
// queries (see "Software-EGL-over-GBM fallback" Step 4), so globbing
// /dev/dri/renderD* directly is the more robust probe.
```

GBM — `GbmDevice` is a re-export: `pub use gbm::{BufferObjectFlags as
GbmBufferFlags, Device as GbmDevice};` (allocator/gbm.rs:15). gbm crate 0.18.0:

```rust
pub struct Device<T: AsFd> { /* private */ }   // gbm-0.18.0 src/device.rs:23
pub fn new(fd: T) -> IoResult<Device<T>>       // device.rs:77 — THE ONLY constructor
```

There is **no** `Device::main()` / `open()` in gbm 0.18 (older gbm 0.8 API —
see correction above). Open the render node yourself:

```rust
let fd = std::fs::File::open("/dev/dri/renderD128")?;
let gbm_dev = GbmDevice::new(fd)?;              // GbmDevice<File>
// dev_path() for the fd: smithay's blanket trait `DevPath` for all AsFd
// (src/utils/fd.rs:61-71) reads /proc/self/fd/<n> — gbm_dev.dev_path()
```

EGL display / context / renderer:

```rust
// EGLDisplay — egl/display.rs
pub unsafe fn new<N>(native: N) -> Result<EGLDisplay, Error>
where N: EGLNativeDisplay + 'static                        // egl/display.rs:201

// impl EGLNativeDisplay for GbmDevice<A: AsFd + Send + 'static>
// (egl/native.rs:147, gated backend_gbm) — tries EGL_KHR_platform_gbm
// then EGL_MESA_platform_gbm with the raw gbm_device pointer.

// EGLContext — egl/context.rs
pub fn new(display: &EGLDisplay) -> Result<EGLContext, Error>   // context.rs:119
pub fn display(&self) -> &EGLDisplay                            // context.rs:459

// GlesRenderer — gles/mod.rs (module gated by feature `renderer_gl`, NOT
// `renderer_glow` — see correction in the TL;DR)
pub unsafe fn new(context: EGLContext) -> Result<GlesRenderer, GlesError>   // gles/mod.rs:468
pub unsafe fn with_capabilities(context: EGLContext, capabilities: impl IntoIterator<Item = Capability>)
    -> Result<GlesRenderer, GlesError>                                 // gles/mod.rs:491
pub fn egl_context(&self) -> &EGLContext                               // gles/mod.rs:1796

// ImportEgl (gated wayland_frontend+backend_egl+use_system_lib) — needed for
// wl_drm/EGL buffers ONLY, not for zwp_linux_dmabuf:
fn bind_wl_display(&mut self, display: &wayland_server::DisplayHandle)
    -> Result<(), crate::backend::egl::Error>     // gles/mod.rs:1098
```

dmabuf format set + dev_t:

```rust
// EGLDisplay — egl/display.rs (mirrored on EGLContext, context.rs:464/469)
pub fn dmabuf_render_formats(&self) -> &FormatSet   // display.rs:593
pub fn dmabuf_texture_formats(&self) -> &FormatSet  // display.rs:598

// FormatSet = smithay::backend::allocator::format::FormatSet
// (Arc<IndexSet<Format>>; .iter() yields &Format; FromIterator<Format>).
// Format = drm_fourcc::DrmFormat { code: Fourcc, modifier: Modifier }
// (re-exported at allocator/mod.rs:38).
// So: let formats: Vec<Format> = display.dmabuf_render_formats().iter().copied().collect();

// DrmNode — drm 0.14.0, re-exported as smithay::backend::drm::DrmNode
// (backend/drm/mod.rs:94)
DrmNode::from_path(path)            // -> Result<DrmNode, CreateDrmNodeError>
node.dev_id()                       // -> dev_t  (rustix u64; == libc dev_t value on linux)
node.dev_path()                     // -> Option<PathBuf>
node.node_with_type(NodeType::Render) // -> Option<Result<DrmNode, CreateDrmNodeError>>
// main_device for the feedback global =
//   DrmNode::from_path("/dev/dri/renderD128")?.dev_id()   (or stat the node, st_rdev)
```

### DmabufState / DmabufGlobal / DmaBufHandler (verbatim, wayland/dmabuf/mod.rs)

Global creation — **`DmabufGlobalState` is a PRIVATE struct (mod.rs:568) in
0.7.0; there is no public `DmabufGlobalState::new`.** Use:

```rust
pub struct DmabufFeedbackBuilder;                          // mod.rs:258
pub fn new(main_device: libc::dev_t,
           formats: impl IntoIterator<Item = Format>) -> Self   // mod.rs:315
pub fn add_preference_tranche(self, target_device: libc::dev_t,
           flags: Option<zwp_linux_dmabuf_feedback_v1::TrancheFlags>,
           formats: impl IntoIterator<Item = Format>) -> Self   // mod.rs:341
pub fn build(mut self) -> Result<DmabufFeedback, std::io::Error> // mod.rs:390

pub struct DmabufState { /* globals: HashMap<usize, DmabufGlobalState> */ }  // mod.rs:580
pub fn new() -> DmabufState                                          // mod.rs:588

// v5 global with default feedback (recommended):
pub fn create_global_with_default_feedback<D>(
    &mut self,
    display: &DisplayHandle,
    default_feedback: &DmabufFeedback,
) -> DmabufGlobal
where
    D: GlobalDispatch<zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1, DmabufGlobalData>
        + BufferHandler
        + DmabufHandler
        + 'static                                                      // mod.rs:645

// Simpler v3 global, formats only (no dev_t/feedback needed):
pub fn create_global<D>(
    &mut self,
    display: &DisplayHandle,
    formats: impl IntoIterator<Item = Format>,
) -> DmabufGlobal
where
    D: GlobalDispatch<zwp_linux_dmabuf_v1::ZwpLinuxDmabufV1, DmabufGlobalData>
        + BufferHandler + DmabufHandler + 'static                      // mod.rs:598
```

`create_global*` registers the global on the `DisplayHandle` internally
(`display.create_global::<D, ZwpLinuxDmabufV1, _>(version, data)` at
mod.rs:733 — version 5 when feedback is passed, 3 otherwise). No manual
`create_global` call is needed in user code.

```rust
pub trait DmabufHandler: BufferHandler {                    // mod.rs:992
    fn dmabuf_state(&mut self) -> &mut DmabufState;         // mod.rs:994
    fn dmabuf_imported(&mut self, global: &DmabufGlobal,
                       dmabuf: Dmabuf, notifier: ImportNotifier);  // mod.rs:1002
    fn new_surface_feedback(&mut self, _surface: &WlSurface,
                       _global: &DmabufGlobal)
        -> Option<DmabufFeedback> { None }                  // mod.rs:1011 (has default)
}

// ImportNotifier (mod.rs:854):
pub fn successful<D>(mut self) -> Result<WlBuffer, InvalidId>   // mod.rs:881
    where D: Dispatch<zwp_linux_buffer_params_v1::ZwpLinuxBufferParamsV1, DmabufParamsData>
             + Dispatch<wl_buffer::WlBuffer, Dmabuf>
             + BufferHandler + DmabufHandler + 'static
pub fn failed(mut self)                                         // mod.rs:957
pub fn incomplete(mut self) / invalid_dimensions / invalid_format  // mod.rs:928/937/948
pub fn client(&self) -> Option<Client>                          // mod.rs:874
```

`dmabuf_imported` must: import the dmabuf into the renderer
(`renderer.import_dmabuf(&dmabuf, None)` for Gles; or `import_buffer` after
wrapping), then `let _ = notifier.successful::<State>();` on success or
`notifier.failed()` on failure (dropping the notifier without a reply logs a
"Compositor bug" warning, mod.rs:980-988).

```rust
macro_rules! delegate_dmabuf { ($(@<...>)? $ty:ty) => { ... } }  // mod.rs:1037
// delegates ZwpLinuxDmabufV1 global dispatch + ZwpLinuxDmabufV1 /
// ZwpLinuxBufferParamsV1 / WlBuffer(=Dmabuf data) / ZwpLinuxDmabufFeedbackV1
// dispatch to DmabufState. Call as `delegate_dmabuf!(State);`
pub fn get_dmabuf(buffer: &wl_buffer::WlBuffer) -> Result<&Dmabuf, UnmanagedResource>  // mod.rs:1029
```

### OffscreenRenderer generalization — every Pixman reference to change

`crates/server/src/rendering/mod.rs` (line numbers as of 2026-08-21):

| Line | Current | Becomes |
| --- | --- | --- |
| 10 | `use smithay::backend::renderer::pixman::{PixmanRenderer, PixmanTexture};` | drop (traits import at 11-13 already covers everything used) |
| 89 | `renderer: PixmanRenderer,` (struct field) | `renderer: R,` in `pub struct OffscreenRenderer<R>` |
| 97 | `let renderer = PixmanRenderer::new()?;` | per-backend ctors (e.g. `new_pixman(w,h)` / `new_gles(renderer, w,h)`) |
| 114 | `let mut textures: Vec<(PixmanTexture, i32, i32)>` | `Vec<(R::TextureId, i32, i32)>` |
| 281 | `let mut textures: Vec<(PixmanTexture, i32, i32)>` | `Vec<(R::TextureId, i32, i32)>` |
| 291 | `let cursor_tex: Option<(PixmanTexture, Point<i32, Logical>)>` | `Option<(R::TextureId, Point<i32, Logical>)>` |
| 373 | `fn committed_texture(...) -> Option<PixmanTexture>` | `-> Option<R::TextureId>` |
| 1, 3, 87, 132, 208, 257, 307 | doc comments mentioning "pixman" | reword (pixman renderer clips... → renderer clips...) |

That is ALL of them. Everything else (`FrameBuffer`, `RenderRequest`,
`collect_surfaces`, and every render-pass call: `import_buffer`,
`create_buffer`, `bind`, `render`, `clear`, `render_texture_at`, `finish`,
`wait`, `copy_framebuffer`, `map_texture`) already goes through the shared
traits — the generalization is mechanical. Two structural notes:

- `Offscreen<Target>` is generic in smithay, so the offscreen buffer type
  must be named. Since the buffer (`image`) is method-local, make **each
  render method generic in it**:
  `fn render<P>(&mut self, ...) where R: ImportAll + ExportMem + Offscreen<P>`
  (`Offscreen<P>` implies `Renderer + Bind<P>`). The struct itself only needs
  `R`: `pub struct OffscreenRenderer<R> { renderer: R, width: u32, height: u32 }`.
- Call sites to update: `crates/server/src/state.rs:264`
  (`pub renderer: Option<OffscreenRenderer>`) and
  `crates/server/src/lib.rs:69` (`OffscreenRenderer::new(w, h)`).

### Feature-flag corrections + implementation plan (probe-and-fallback)

Corrections to earlier sections of this file:
1. **`GlesRenderer` is behind `renderer_gl`**, not `renderer_glow`
   (renderer/mod.rs:27-28: `#[cfg(feature = "renderer_gl")] pub mod gles;`).
   `renderer_glow` adds a *different* `GlowRenderer` (renderer/glow.rs).
2. `gbm 0.18` has no `Device::main()` — only `Device::new(fd)`.
3. No public `DmabufGlobalState::new` — use `DmabufState::create_global*`.

Feature changes (workspace `Cargo.toml:16`, currently
`features = ["wayland_frontend", "renderer_pixman"]`):
- Add **`renderer_gl`** (→ `gl_generator` + `backend_egl`; EGL loaded at
  runtime via libloading, so no `libegl-dev` needed to build).
- Add **`backend_gbm`** (→ `gbm` crate + `backend_drm`; needs `libgbm-dev` +
  `libdrm-dev` at build time on Linux; NOT installed on gary-agents).
- Keep `renderer_pixman` (fallback).
- **`use_system_lib` NOT required for the dmabuf path.** Without it, the
  `ImportAll` blanket is `Renderer + ImportMemWl + ImportDmaWl`
  (renderer/mod.rs:704) — handles wl_shm + zwp_linux_dmabuf, which is exactly
  what we need (weston-simple-egl uses zwp_linux_dmabuf). It IS required for
  wl_drm/EGL-buffer import (`ImportEgl for GlesRenderer`, gles/mod.rs:1092) —
  and note it flips `wayland-backend` to system `libwayland-server` across the
  whole build (feature unification).

Probe-and-fallback structure:

```text
fn build_renderer(display_handle: &DisplayHandle)
    -> (OffscreenRenderer<R>, Option<GlesDmabufSetup>)  // setup = (dev_t, Vec<Format>)
  1. Probe Gles path; on ANY failure log + fall back to pixman:
       for each /dev/dri/renderD*:
         std::fs::File::open(path)
         GbmDevice::new(fd)                                  // gbm 0.18
         unsafe { EGLDisplay::new(gbm_dev)? }                // egl/display.rs:201
         EGLContext::new(&display)?                          // egl/context.rs:119
         unsafe { GlesRenderer::new(context)? }              // gles/mod.rs:468
         if display.dmabuf_render_formats() is empty -> treat as failure
         (optional) renderer.bind_wl_display(display_handle) // only w/ use_system_lib
  2. Gles ok  -> OffscreenRenderer::<GlesRenderer>::new_gles(renderer, w, h)
                 + dmabuf global: DmabufState::new()
                 + DmabufFeedbackBuilder::new(dev_t, formats.iter().copied()).build()?
                 + dmabuf_state.create_global_with_default_feedback::<State>(&dh, &fb)
                 dev_t = DrmNode::from_path(path)?.dev_id()
                 formats = display.dmabuf_render_formats()
  3. Gles fail -> OffscreenRenderer::<PixmanRenderer>::new_pixman(w, h), NO dmabuf global
```

Files that change:
- `Cargo.toml` (workspace) — smithay features (see above).
- `crates/server/src/rendering/mod.rs` — generic `OffscreenRenderer<R>` +
  `new_pixman`/`new_gles` ctors (the mechanical changes listed above).
- `crates/server/src/rendering/egl.rs` (new) — the probe chain + format/dev_t
  collection (keeps lib.rs small).
- `crates/server/src/state.rs` — `dmabuf_state: DmabufState` field (created
  unconditionally in `State::new`, cheap), `dmabuf_global: Option<DmabufGlobal>`
  (set only on the Gles path), `impl DmabufHandler for State`
  (`dmabuf_imported` → `self.renderer`'s `import_dmabuf` + notifier reply),
  `delegate_dmabuf!(State);` next to the other delegates (state.rs:638-645).
- `crates/server/src/lib.rs` — replace the `OffscreenRenderer::new` call at
  :69 with the probe; register the dmabuf global right after `State::new`
  (:60) since `DmabufHandler` is implemented by `State`.

Blockers/surprises that shape the design:
- `EGLDisplay::new` and `GlesRenderer::new` are `unsafe` (context must not be
  current on another thread) — wrap in `unsafe` blocks with the safety comment.
- `GlesRenderer` is `!Send` — fine, `State` never leaves the compositor thread.
- `bind_wl_display` is NOT needed for zwp_linux_dmabuf (that's the wl_drm/EGL
  path) — earlier notes saying "must be called for any dmabuf import" are
  wrong; it only matters if we later want wl_drm buffers (needs `use_system_lib`).
- Pixman fallback *could* advertise a dmabuf global (it implements `ImportDma`
  @1167 via CPU mapping, linear formats only) — NOT advertising it on the
  pixman path is a design choice (MVP semantics), not an API constraint.
- rustix `dev_t` (from `DrmNode::dev_id`) vs `libc::dev_t` (feedback builder
  param) are both `u64` on linux — value-identical, cast if the compiler
  complains about distinct aliases.
- `Offscreen<GlesTexture>::create_buffer` only accepts RGBA8/BGRA_EXT/10-bit
  formats — Argb8888 maps to BGRA_EXT and is fine (gles/format.rs:21).
- On gary-agents, `EGLDevice::enumerate()`/string queries degrade (glvnd
  returns null per-device strings — see Step 4 above), so glob
  `/dev/dri/renderD*` rather than relying on `try_get_render_node`.

## Current server import limit (pixman/shm only)

- `crates/server/src/state.rs` — delegates: `compositor, shm, seat, output,
  xdg_shell, viewporter, data_device, text_input_manager`. **No `delegate_dmabuf`, no
  `DmabufState`, no `DmaBufHandler`.**
- `crates/server/src/rendering/mod.rs` — `OffscreenRenderer { renderer: PixmanRenderer,
  ... }`. All three render paths (`render`, `render_surface`, `render_window_surface`)
  call `self.renderer.import_buffer(buffer, None, &[])`. `PixmanRenderer` only imports
  **wl_shm** buffers. No EGL, no GBM, no dmabuf path exists.
- So `weston-simple-egl` (EGL-only, no shm fallback) currently has nothing to hand its
  buffers to — its dmabuf/EGL buffers are simply not imported.

## Software-EGL / headless feasibility (honest assessment)

**Path A — surfaceless EGL (llvmpipe), no /dev/dri:**
`EGLDisplay::new(EGLSurfacelessDisplay)` + `EGLContext::new` CAN succeed (no device).
But it **cannot import dmabuf**: a surfaceless display does not advertise
`EGL_EXT_image_dma_buf_import`, so `get_dmabuf_formats` returns empty sets and
`create_image_from_dmabuf` / `GlesRenderer::import_dmabuf` fail. You get a GL context
for offscreen rendering (pixman's job anyway), not a dmabuf import path. **Dead end
for the actual goal.**

**Path B — GBM over swrast (swrast_dri.so present):** GBM fundamentally needs a DRM
device (`GbmDevice` opens a render node). `swrast_dri.so` is a GL DRI driver, not a GBM
device provider, and a GPU-less cloud VM has no DRM KMS/render device to bind. So GBM
does not come up without a real (or virtual) DRM device. **Blocked on no-GPU.**

**Path C — wgpu / lavapipe (software Vulkan):** lavapipe IS installed
(`lvp_icd.json`, `mesa-vulkan-drivers`, `libvulkan1`). But **smithay 0.7.0 has no wgpu
renderer** (no `renderer_wgpu` feature, no wgpu in `src/`; `backend_vulkan` is only the
Vulkan allocator). So there is no smithay compositor renderer to drive through lavapipe
in 0.7.0. **Not available in this smithay version.**

**Client-side reality:** the EGL *client* (weston-simple-egl / GL apps) must itself
allocate dmabuf buffers via EGL+GBM, which needs a render node. On a GPU-less box the
client cannot run at all, regardless of what the compositor supports.

**Net:** a *software-only* EGL/dmabuf compositor is **not achievable in smithay 0.7.0 on
a GPU-less headless box.** Supporting EGL/dmabuf clients requires a render node (real or
virtual GPU) on the box. On gary-agents (which has 3x RTX 5060 Ti) that path works and is
a normal feature; on a truly headless no-GPU VM it is a hard blocker until either (a) we
provision GPUs/vGPUs, or (b) a smithay wgpu/lavapipe renderer lands.

## Software-EGL-over-GBM fallback (llvmpipe)

**Verdict: NO — on gary-agents a GBM-backed *software* (llvmpipe/swrast) EGL
display cannot be brought up at all, so it never reaches the point of
advertising (or using) `EGL_EXT_image_dma_buf_import`.** The hypothesis
"llvmpipe-over-GBM works on any box that has a DRM render node" is **NOT
supported** by this box and is specifically broken when the render node is a
vendor (NVIDIA) device. The only working dmabuf-import path on gary-agents is
the NVIDIA **hardware** EGL path. Details + exact outputs below.

Method note: `sudo apt-get install -y mesa-utils` requires interactive password
auth (not available in this session), so `eglinfo` could not be installed.
Instead a self-contained C probe (`/tmp/egl_gbm_probe.c`, compiled with gcc)
dlopens `libEGL.so.1` + `libgbm.so.1`, declares the EGL prototypes itself,
opens a render node, builds a `gbm_device`, creates a GBM platform display
(`eglGetPlatformDisplay(EGL_PLATFORM_GBM_MESA=0x31D7, gbm_dev, NULL)`),
initializes it, dumps vendor/version/extensions, creates a context for
GL_VENDOR/RENDERER, and enumerates EGLDevices. Run via `ssh gary-agents`.

### Step 2 — HW mode (NVIDIA ICD, GBM): works, dmabuf advertised

```
$ /tmp/egl_gbm_probe /dev/dri/renderD128
EGL initialized: 1.5
EGL_VENDOR     : NVIDIA
EGL_VERSION    : 1.5
EGL_CLIENT_APIS: OpenGL_ES OpenGL
EGL_EXTENSIONS: ... EGL_EXT_image_dma_buf_import EGL_EXT_image_dma_buf_import_modifiers
                EGL_MESA_image_dma_buf_export ... (full NVIDIA ext list)
>>> EGL_EXT_image_dma_buf_import: PRESENT
eglGetConfigs: ok=1 total=65
GL_VENDOR    : NVIDIA Corporation
GL_RENDERER  : NVIDIA GeForce RTX 5060 Ti/PCIe/SSE2
GL_VERSION   : OpenGL ES 1.1 NVIDIA 595.84
```

Yes — this is the NVIDIA (hw) path, and it advertises dmabuf import. (The
NVIDIA ICD is also fragile: a configless `eglCreateContext` segfaults it; a
config from `eglGetConfigs` works and yields the strings above.)

### Step 3 — Software mode (the crux): FAILS

- `LIBGL_ALWAYS_SOFTWARE=1` **alone does NOT force software**: the NVIDIA ICD
  is still selected (`EGL_VENDOR: NVIDIA`) because the proprietary driver
  ignores that env var.
- To force the Mesa ICD you must also restrict the ICD loader:
  `__EGL_VENDOR_LIBRARY_FILENAMES=/usr/share/glvnd/egl_vendor.d/50_mesa.json`.
  With that + `LIBGL_ALWAYS_SOFTWARE=1` — and also with
  `GALLIUM_DRIVER=llvmpipe`, `MESA_LOADER_DRIVER_OVERRIDE=swrast`,
  `__DRI_DRIVER=swrast` — **`eglInitialize` FAILS** (`eglGetError=0x3001`,
  `EGL_NOT_INITIALIZED`) in every variant. No software GBM display comes up.

Root cause (strace): for the NVIDIA render node, libgbm loads the **NVIDIA GBM
backend** — `openat("/usr/lib/x86_64-linux-gnu/gbm/nvidia-drm_gbm.so")` (→
`libnvidia-allocator.so.1`, from `libnvidia-extra-595`) — and Mesa then cannot
attach a software DRI driver on top of it (it even probes `libnvtegrahv.so`,
ENOENT) before `eglInitialize` returns false. Only two GBM backends exist on the
box: the generic `dri_gbm.so` (Mesa) and `nvidia-drm_gbm.so` (NVIDIA). There is
no virtio GBM backend present.

### Software display that DOES init (surfaceless, non-GBM) — still a dead end

`EGL_PLATFORM_SURFACELESS_MESA` + Mesa ICD + `LIBGL_ALWAYS_SOFTWARE=1` +
`MESA_LOADER_DRIVER_OVERRIDE=swrast`:

```
EGL initialized: 1.5
EGL_VENDOR     : Mesa Project
>>> EGL_EXT_image_dma_buf_import: PRESENT      (it IS advertised)
eglGetConfigs: ok=1 total=128                  (configs enumerated)
eglCreateContext: fails for EVERY config
    -> EGL_BAD_CONFIG (0x3003) with no attribs
    -> EGL_BAD_ATTRIBUTE (0x3004) with EGL_CONTEXT_CLIENT_VERSION
=> NO renderable GL context; GL_RENDERER cannot be read.
```

So even the non-GBM software display is a non-functional shell here: it
advertises the dmabuf extension and lists configs, but **cannot create a
context**. (Correction to the earlier TL;DR note: on Mesa 26 a surfaceless
software display DOES advertise `EGL_EXT_image_dma_buf_import` — it is still a
dead end, but for the reason "no context," not "extension not advertised.")

### Step 4 — EGL device enumeration

- NVIDIA (hw) dispatch: `eglQueryDevices` → **num EGLDevices = 7**.
- Mesa swrast (surfaceless): **num EGLDevices = 1**.
- `eglQueryDeviceString` / `eglQueryDeviceAttrib` resolve but return null/0 for
  every device via the glvnd dispatch on this box, so per-device type (GPU vs
  CPU) and name **cannot** be read here. (Counts only.)

### smithay 0.7.0 requirement chain (confirmed from source)

The code is **vendor-agnostic** (as hypothesized) but has a hard precondition
the software path fails on this box:

- `native.rs:147` — `impl EGLNativeDisplay for GbmDevice<A>` tries
  `EGL_KHR_platform_gbm` then `EGL_MESA_platform_gbm` (both value 0x31D7),
  passing the raw `gbm_device` pointer.
- `display.rs` `EGLDisplay::new` (~line 248) — calls `eglInitialize`; on
  failure returns `Error::InitFailed`. **A GBM display that cannot initialize
  is a hard failure** — exactly what happens with Mesa software on the NVIDIA
  nodes above.
- `display.rs` `get_dmabuf_formats` (~line 868) — returns EMPTY
  `dmabuf_render_formats` unless the display advertises
  `EGL_EXT_image_dma_buf_import`; if `EGL_EXT_image_dma_buf_import_modifiers`
  is also present it further requires `eglQueryDmaBufFormatsEXT` to return >0
  formats (else empty). If only the base import extension is present it guesses
  `{Argb8888, Xrgb8888}`. It **never inspects the GL vendor**.

### Implication / updated recommendation

- "Has a DRM render node" is **necessary but not sufficient** for the
  llvmpipe-over-GBM fallback: the node's GBM backend and whether Mesa can attach
  a (software) DRI driver both matter. On a vendor (NVIDIA) render node the
  software GBM path does not come up at all.
- The fallback *might* still work on a VM whose render node uses the **generic
  `dri` GBM backend** (e.g. virtio-gpu / an open-source GPU) — that scenario is
  **not testable on gary-agents** (no such device present) and remains
  **UNVERIFIED**.
- The **hw-EGL** path (NVIDIA here) is the reliable, working dmabuf-import
  route on this box.
- Recommendation: keep the "llvmpipe-over-GBM works anywhere" claim
  **DEFERred / unproven**. The dmabuf/EGL feature is **IN SCOPE** for
  deployment boxes with a working GPU driver (execute the scoped plan in
  "What it would take"). If the production target is a GPU-less / virtio-only
  VM, verify the software-GBM path on an **actual virtio-gpu VM** (or wait for
  a smithay wgpu/lavapipe renderer) before promising EGL/dmabuf clients there.

## What it would take (GPU-backed box, e.g. gary-agents)

1. `Cargo.toml`: add `renderer_gl`, `backend_gbm` to smithay features (keep
   `wayland_frontend`, optionally keep `renderer_pixman` as fallback).
2. `apt-get install libgbm-dev libdrm-dev` (build deps; EGL itself is loaded at runtime
   via libloading so `libegl-dev` is not strictly required to compile).
3. Runtime: pick a render node (`GbmDevice::new("/dev/dri/renderD128")` or
   `EGLDevice::enumerate()`), `EGLDisplay::new(gbm_device)`, `EGLContext::new`,
   `GlesRenderer::new(ctx)`, `renderer.bind_wl_display(handle)`.
4. `state.rs`: add `DmabufState`, a `DmabufGlobalState::new(main_device, formats)`,
   implement `DmaBufHandler` (import in `dmabuf_imported`), `delegate_dmabuf!(State)`,
   register the global.
5. Format negotiation: advertise the intersection of the display's
   `dmabuf_render_formats` and what the GBM device supports; handle
   `is_external`/modifier fallback.
6. Cost estimate: moderate — feature flags + ~200–300 lines of wiring (state,
   DmaBufHandler, delegate, renderer setup) + 2 build-dep packages. Not a one-liner,
   and it competes with the other 88–92% GPU workloads on gary-agents.

## Decision recommendation

**Defer implementation now; keep the above as the scoped follow-up plan; gate it on a
deployment-hardware decision.** Rationale: the whole premise of issue 08 was a "headless
no-GPU VM," but gary-agents actually has three NVIDIA GPUs, so the EGL/dmabuf path is
demonstrably feasible *there* — yet it is a real feature (feature flags + `DmabufState`
wiring + `GbmDevice` + `GlesRenderer` + format negotiation + `libgbm-dev`/`libdrm-dev`),
not a small change, and the GPUs are already 88–92% busy. More decisively, for the
intended *production* target (a headless, GPU-less VM) smithay 0.7.0 has **no software
path to import dmabuf** — surfaceless EGL can't (no context on gary-agents), GBM needs a
render node, there is no wgpu/lavapipe renderer, **and the "llvmpipe-over-GBM" fallback
was specifically tested and does NOT work on vendor (NVIDIA) render nodes** (Mesa
`eglInitialize` fails; see "Software-EGL-over-GBM fallback (llvmpipe)") — so this work
would not actually unblock EGL clients like `weston-simple-egl` in a no-GPU environment.
The llvmpipe-GBM fallback remains unverified (not testable here) only for a virtio-gpu /
generic-`dri`-backend VM. The MVP is pixman/shm and already covers the
MVP test story (see `lat.md/decisions#Renderer`). The right next step is a decision, not
code: **do we standardize on GPU-backed deployment boxes?** If yes, execute the scoped
plan above (the NVIDIA hw-EGL path works and is the reliable route). If the target stays
GPU-less / virtio-only, this stays deferred pending either verification of the
software-GBM path on an actual virtio-gpu VM or a smithay wgpu/lavapipe renderer. This
skill file is the scoping artifact so the follow-up doesn't re-research any of the above.

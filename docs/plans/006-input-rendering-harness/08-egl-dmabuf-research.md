# 08 — EGL/dmabuf Feasibility (Research + Decision)

## Objective

Determine whether GPU/EGL clients (`weston-simple-egl`, GL apps) can be
supported in this headless, software-rendering compositor, and either scope a
follow-up plan or document a deferral with rationale. This issue is **research
only** — no production code unless the decision is "in scope."

## Files

| File | Change |
|------|--------|
| `.agents/skills/egl-dmabuf-feasibility/SKILL.md` (new) | Findings: smithay dmabuf support surface, software EGL (llvmpipe/ mesa swrast) options, what the pixman renderer can import, the cost of a `DmabufState` + GBM/EGL backend on a headless Linux box. |
| `docs/plans/006-input-rendering-harness/08-egl-dmabuf-research.md` | Append the decision (defer / follow-up plan number) at the end. |

## Steps

1. Investigate on gary-agents: what's available — `eglinfo`, mesa, llvmpipe/swrast, `/dev/dri` render nodes (none expected on a headless VM). Check smithay's `DmaBuf` / `egl::EglContext` / `gbm` backend requirements.
2. Determine whether the pixman software renderer can import dmabuf buffers (it cannot import EGL/dmabuf — only `wl_shm`). So EGL clients would need a separate EGL import path and a GPU or software-EGL.
3. Evaluate software-EGL (llvmpipe) feasibility: mesa + `EGL_MESA_platform_surfaceless` / GBM on a render node. Assess install cost and whether smithay's `renderer_egl` + `allocator_dmabuf` compile/run headless.
4. Decide: if software-EGL is installable and smithay supports it, write a follow-up plan stub (issues: add `DmabufState` global, EGL backend, import path, fall back to shm). If not practical, document the deferral: "EGL/GPU clients require a GPU or a heavy software-EGL stack; out of scope for the headless streaming server."
5. Write `.agents/skills/egl-dmabuf-feasibility/SKILL.md` with the exact findings (versions, commands tried, smithay API surface) so future agents don't re-research.

## Verification

- A decision is recorded in this issue file (defer or follow-up plan number) with a one-paragraph rationale.
- The skill file exists and is accurate (AGENTS.md: document uncommon APIs to skill files).
- `lat check` green (no code change expected; if deferred, only docs/skill files).

## Decision

**IN SCOPE** — implement the EGL/dmabuf import path for deployment boxes that
have a working GL/DRM render node (a real GPU, or a virtual one like
virtio-gpu driven by llvmpipe). Refines the earlier DEFER after a software-EGL
probe.

The premise correction: a box that runs a real Wayland session has a DRM render
node, so the "GPU-less, no render device" case is not the target. The
compositor's EGL code is **hw/software agnostic** — it binds GBM to whatever
render node exists and Mesa picks the GL driver (hw if present, llvmpipe
otherwise). The implementation does not branch on hw vs software.

Verified on gary-agents (3× NVIDIA RTX 5060 Ti):
- **hw-EGL path works** — `EGL_PLATFORM=gbm eglinfo` reports `NVIDIA / GeForce
  RTX 5060 Ti` and advertises `EGL_EXT_image_dma_buf_import`. This is the
  reliable route and is what we'll exercise with `weston-simple-egl`.
- **Forced-software over the NVIDIA node fails** (`eglInitialize` 0x3001):
  libgbm loads the vendor `nvidia-drm_gbm.so` backend and Mesa cannot attach a
  software DRI driver on top of it. This is an **irrelevant config** — you don't
  force llvmpipe on a box whose NVIDIA driver already works.
- The pure-software case the user is asking about (llvmpipe-over-GBM) runs on a
  **generic-DRM device** (e.g. virtio-gpu, the standard headless-VM GPU), which
  uses the generic `dri_gbm.so` backend and *should* accept llvmpipe. That
  scenario is **not testable on gary-agents** (no virtio device present) and
  remains **UNVERIFIED** — but the code path is identical to the hw one.

Cost: moderate — feature flags `renderer_glow` + `backend_gbm`, `DmabufState`
global + `DmaBufHandler` impl + `delegate_dmabuf!`, `GbmDevice`/`EGLDisplay`/
`GlesRenderer` setup, format negotiation; ~200–300 lines + `libgbm-dev`/
`libdrm-dev`. gary-agents' GPUs are 88–92% busy, so testing there contends with
other workloads (the shm path is unaffected).

Follow-up scope (recorded in
`.agents/skills/egl-dmabuf-feasibility/SKILL.md`, section "What it would take"):
add `zwp_linux_dmabuf` global, an EGL/GL backend that imports dmabuf, and keep
the existing pixman/`wl_shm` path as the fallback for boxes with no usable
render node. **Caveat to carry:** if a production target is a virtio-gpu-only
VM, validate the software-GBM path on an actual virtio VM before promising
EGL/dmabuf clients there (or wait for a smithay wgpu/lavapipe renderer).

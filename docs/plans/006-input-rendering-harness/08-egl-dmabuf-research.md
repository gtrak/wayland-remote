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

**DEFER** implementation, pending a deployment-hardware decision.

Rationale: the research overturned the issue's core premise — gary-agents is
not a GPU-less headless VM; it has three NVIDIA RTX 5060 Ti GPUs with render
nodes, so the EGL/dmabuf path is demonstrably feasible *there*. However, it is
a moderate feature (feature flags `renderer_glow` + `backend_gbm`, a `DmabufState`
global + `DmaBufHandler` impl + `delegate_dmabuf!`, `GbmDevice`/`EGLDisplay`/
`GlesRenderer` setup, and format negotiation — ~200–300 lines plus `libgbm-dev`
/`libdrm-dev`), and those GPUs are already 88–92% utilized by other workloads.
Decisively, for the stated production target (a headless, GPU-less box) smithay
0.7.0 has **no software dmabuf import path**: surfaceless EGL cannot import
dmabuf (no `EGL_EXT_image_dma_buf_import`), GBM requires a DRM render node, and
there is no `renderer_wgpu`/lavapipe renderer in 0.7.0 — so this work would not
actually unblock EGL clients (e.g. `weston-simple-egl`, which itself needs a
render node to allocate its buffers) on a GPU-less deployment. The MVP remains
pixman/`wl_shm`, which already covers the MVP test story
(`lat.md/decisions#Renderer`).

The blocking question is a product/deployment call, not a code call: **do we
standardize on GPU-backed deployment boxes?** If yes, execute the scoped plan
recorded in `.agents/skills/egl-dmabuf-feasibility/SKILL.md` (section "What it
would take"). If the target stays GPU-less, this stays deferred until a smithay
wgpu/lavapipe renderer lands. All findings (environment probe, exact smithay
0.7.0 API surface, per-path feasibility verdict, cost estimate) are in that
skill file so the follow-up does not re-research.

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

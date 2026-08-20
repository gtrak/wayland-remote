# Issue 04 — lat.md updates, plan archival, closeout

## Objective

Make the repository legible after the [[005-windows-client-e2e|Plan 005]] build-out per the AGENTS.md post-task checklist and the plan-process skill: complete the `lat.md/` knowledge graph for the viewer and per-window pipeline, add the missing test specs, run `lat check`, and archive plans 002/003/005 (005 completed 002 and 003).

## Files

| File | Change |
|---|---|
| `lat.md/architecture.md` | Add a **Viewer** subsection: net-task/UI-thread split, per-window `FrameStore` map, `PostMessageW` invalidation contract, GDI `StretchDIBits` (top-down BGRA) blit path, controller-HWND-owns-loop / child-HWND-per-window model. Update **Rendering Pipeline** to describe per-window render targets and `window_id` on the wire. |
| `lat.md/decisions.md` | Add Decision Log entries: **Stretch-to-fit resize** (chose stretch over letterbox for M2/M3 simplicity; note the tradeoff); **Per-window render targets** (one pixman target per mapped window instead of one composite desktop; `window_id` on `FrameHeader` is the demux key). |
| `lat.md/tests.md` | Add leaf specs: per-window frame `window_id` (streamed frame carries the created toplevel's id); `Resized` on re-commit (mapped window re-committed larger emits `WindowEventKind::Resized`). Audit existing Viewer/Input specs — confirm each has exactly one `// @lat:` ref in code; add the new ones with refs in the new/updated tests. |
| Source files touched in 01/02 | Add/adjust `// @lat: [[...]]` comments next to the tests that cover the new specs (one per spec). |
| `docs/plans/archive/002-m2-windows-viewer-input.md` | New 20–30 line summary (created at closeout). |
| `docs/plans/archive/003-m3-window-mapping.md` | New 20–30 line summary. |
| `docs/plans/archive/005-windows-client-e2e.md` | New 20–30 line summary including the E2E results from [[005-windows-client-e2e/03-e2e-test|Issue 03]]. |
| `docs/plans/002-m2-windows-viewer-input/`, `docs/plans/003-m3-window-mapping/`, `docs/plans/005-windows-client-e2e/` | Remove after the archive summaries are written. |

## Implementation notes

- Every `lat.md` section needs a leading paragraph ≤250 chars before any child heading.
- Leaf test spec sections require a `// @lat:` (or `# @lat:`) comment in code, exactly one per spec section, placed next to the covering test — not at the top of the file. `lat check` flags any spec without a ref and any ref pointing at a nonexistent section.
- Archive summaries: 1–2 paragraphs (what + why), brief scope bullets, phase/issue list as headings only. Full detail stays in git history.

## Steps

1. Edit `lat.md/architecture.md` (Viewer section + Rendering Pipeline update).
2. Edit `lat.md/decisions.md` (two new Decision Log entries).
3. Edit `lat.md/tests.md` (new leaf specs; audit existing Viewer/Input specs).
4. Add/adjust `// @lat:` refs in the new/updated server and viewer tests.
5. Run `lat check` — must be green with zero warnings.
6. Write the three archive summaries; remove the three plan folders.
7. Final `git add -A && git status` review; leave the commit to the supervisor.

## Verification

- `lat check` green, zero warnings.
- Every leaf spec in `lat.md/tests.md` has exactly one `// @lat:` ref in code; every `// @lat:` ref points at an existing section.
- `docs/plans/` contains only `archive/`; the three archive summaries are each 20–30 lines.
- A fresh agent, given only README + `lat.md/`, can orient without reading git history.

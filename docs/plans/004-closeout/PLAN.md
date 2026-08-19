# Plan 004 — Closeout: docs, lat.md completion, archive

## Why

MVP is functionally complete after [[003-m3-window-mapping|M3]]. This plan makes the repository legible to newcomers and future agents: complete the `lat.md/` knowledge graph (every test spec has its `@lat:` ref), write the README quickstart, and archive the plan folders per the plan-process skill.

## What

- Audit `lat.md/` against the implementation; fill gaps (every crate's architecture section, decision log entries for anything decided during implementation that wasn't pre-seeded).
- Verify every test in the workspace maps to a leaf spec section with exactly one `// @lat:` comment, and every spec section maps to a test.
- README: what it is (one paragraph), quickstart (server, viewer, ffplay via wr-dump, fingerprint TOFU), development (deps, CI, cross-compile), current limitations pointing at PRD §7 future work.
- Archive plans 001–004 into `docs/plans/archive/` summaries.
- Tag `v0.1.0` (release workflow already exists).

## Success criteria

- `lat check` green with zero warnings; spec↔test audit table shows full coverage.
- README rendered sensible; quickstart commands copy-pasteable and verified by running them.
- `docs/plans/` contains only `archive/`; each archived summary is 20-30 lines per the skill.
- CI fully green on the tag; release artifacts (server binary, viewer exe) attached.
- A fresh agent, given only README + `lat.md/`, can orient without reading git history.

## Task order

```
01-lat-readme-archive
```

## Scope

In: documentation, spec reconciliation, archival, release tagging.

Out: any new features, refactors, or PRD §7 performance work (dmabuf, video codecs, damage networking, WAN/lossy tuning — these become new plan folders when prioritized).

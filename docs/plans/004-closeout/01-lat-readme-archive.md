# Issue 01 — lat.md completion, README, archive

## Objective

Reconcile the `lat.md/` knowledge graph with the shipped implementation, write the README, archive all plan folders, and cut `v0.1.0`.

## Files

| File | Change |
|---|---|
| `lat.md/architecture.md` | Full system + per-crate sections: compositor loop, runtime split, rendering pipeline, QUIC session model, viewer structure, window mapping — every section linked to code symbols (`[[crates/server/src/net/session.rs#NetServer]]` style) |
| `lat.md/decisions.md` | Complete decision log incl. anything decided during implementation (this is an audit: diff what's documented vs. what the code does) |
| `lat.md/tests.md` | Final spec tree; frontmatter `require-code-mention: true` |
| every test file | Exactly one `// @lat: [[tests#...]]` comment next to each test, verified both directions |
| `README.md` | New (see steps) |
| `docs/plans/archive/001..004-*.md` | Archive summaries; remove active folders |
| source files | `// @lat:` refs at key symbols tying implementation to architecture sections |

## Steps

1. **Spec reconciliation**: run `lat check`; list all errors. Then walk every test file and every `lat.md/tests.md` leaf — build a two-column table (test fn ↔ spec section). Fix gaps in both directions: missing specs written, orphan specs deleted, duplicate `@lat:` refs removed. Re-run `lat check` until clean.
2. **Architecture completion**: walk each crate's modules; ensure every major symbol (public types, `main`, handler impls) is reachable from a `lat.md` section via wiki link, and conversely that `architecture.md` has no sections describing things that don't exist. Use the `lat refs` command to find dangling links.
3. **Decision log audit**: read `git log` diffs for decisions made mid-implementation (anything not in the seeded decision log); add entries with rationale.
4. **README**: structure — intro (1 para: Wayland compositor streaming to Windows, why it's not VNC/waypipe); architecture diagram (ASCII, from PLAN 001); quickstart: install deps (apt line), build server, run server + test client, `wr-dump | ffplay` one-liner, Windows viewer (fingerprint TOFU flow); development: CI jobs, zigbuild, running tests, lat.md/ conventions pointer; limitations + roadmap (PRD §7 items).
5. **Verify quickstart**: literally execute every README command in a clean shell; fix what's wrong.
6. **Archive**: write `docs/plans/archive/00N-<name>.md` summaries (20-30 lines each: what/why, scope bullets, task list headings) for plans 001-004; delete the active folders.
7. **Release**: confirm CI green on main, tag `v0.1.0`, push, verify release artifacts appear (existing release.yml builds server + viewer; if the zigbuild exe isn't wired into release.yml, add that job mirroring CI's).

## Verification

- `lat check` exits 0 with no errors or warnings.
- `lat refs` spot-checks: `[[decisions#Runtime Split]]` referenced from `crates/server/src/bridge.rs`'s `@lat:` comment.
- README commands run verbatim (document the exact session used to verify).
- `ls docs/plans/` shows only `archive/`; four summary files exist.
- Tag pushed; release page has `wayland-remote-server` (Linux) and `wayland-remote-viewer.exe` (Windows).
- Final CI run on the tag: all jobs green.

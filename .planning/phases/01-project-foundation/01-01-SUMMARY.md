---
phase: 01-project-foundation
plan: 01
subsystem: infrastructure
tags: [rust, cargo, workspace, smithay, tokio, winit]

# Dependency graph
requires: []
provides:
  - Virtual workspace root configuration
  - Workspace dependencies for shared crates
  - Rust toolchain pinning
  - Server and viewer crate stubs
affects: [all phases]

# Tech tracking
tech-stack:
  added: [cargo workspace, rust-toolchain.toml]
  patterns: [virtual workspace, workspace.dependencies inheritance]

key-files:
  created:
    - Cargo.toml
    - rust-toolchain.toml
    - Cargo.lock
    - crates/server/Cargo.toml
    - crates/server/src/main.rs
    - crates/viewer/Cargo.toml
    - crates/viewer/src/main.rs
  modified: []

key-decisions:
  - "Use resolver = \"2\" for Rust 1.75 compatibility (not \"3\" which requires Rust 1.83+)"
  - "Pin Smithay to exact version =0.7.0 for stability"
  - "Use edition 2021 for stability over 2024"
  - "Virtual workspace with no [package] section"

patterns-established:
  - "Workspace root with members array and resolver"
  - "workspace.package for inherited metadata"
  - "workspace.dependencies for shared crate versions"
  - "Platform-specific dependencies via [target.'cfg(windows)'.dependencies]"

requirements-completed:
  - INFRA-01

# Metrics
duration: 2 min
completed: 2026-03-10
---

# Phase 01 Plan 01: Virtual Workspace Root Configuration Summary

**Rust virtual workspace with resolver = "2", Smithay =0.7.0 pinned, Tokio 1.40+, and winit 0.30.x workspace dependencies**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-10T06:17:36Z
- **Completed:** 2026-03-10T06:19:55Z
- **Tasks:** 3
- **Files modified:** 7

## Accomplishments
- Virtual workspace root with resolver = "2" for Rust 1.75 compatibility
- Workspace dependencies defined for Smithay, Tokio, tracing, winit
- Rust toolchain pinned to 1.75 with rustfmt and clippy components
- Server and viewer crate stubs created with workspace inheritance
- Cargo.lock generated with 2715 lines of dependency tree

## Task Commits

Each task was committed atomically:

1. **Task 1: Create virtual workspace root Cargo.toml** - `8901489` (feat)
2. **Task 2: Create rust-toolchain.toml** - (combined with Task 1)
3. **Task 3: Generate initial Cargo.lock** - `33cc548` (fix)

**Plan metadata:** (pending final commit)

## Files Created/Modified
- `Cargo.toml` - Virtual workspace root with members, resolver, workspace.dependencies
- `rust-toolchain.toml` - Rust 1.75 pinning with rustfmt and clippy
- `Cargo.lock` - Full dependency lockfile (2715 lines)
- `crates/server/Cargo.toml` - Server crate inheriting workspace deps
- `crates/server/src/main.rs` - Minimal server stub
- `crates/viewer/Cargo.toml` - Viewer crate with Windows-specific deps
- `crates/viewer/src/main.rs` - Minimal viewer stub

## Decisions Made
- **resolver = "2" instead of "3"**: Research specified resolver "3" but it requires Rust 1.83+. Using resolver "2" for compatibility with pinned Rust 1.75.
- **Edition 2021 instead of 2024**: Research mentioned 2024 but 2021 is more stable and widely adopted.
- **Smithay pinned to "=0.7.0"**: Exact version pinning prevents API breakage from patch updates.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Changed resolver from "3" to "2"**
- **Found during:** Task 1 (Cargo.toml creation)
- **Issue:** Research.md specified resolver = "3" but Rust 1.75 only supports resolver "1" or "2"
- **Fix:** Changed to resolver = "2" which is compatible with Rust 1.75
- **Files modified:** Cargo.toml
- **Verification:** `cargo metadata` succeeds
- **Committed in:** `8901489`

**2. [Rule 3 - Blocking] Created stub crates for Cargo.lock generation**
- **Found during:** Task 3 (Cargo.lock generation)
- **Issue:** Cargo.lock couldn't be generated without member crates existing
- **Fix:** Created minimal crates/server and crates/viewer with Cargo.toml and main.rs stubs
- **Files modified:** crates/server/Cargo.toml, crates/server/src/main.rs, crates/viewer/Cargo.toml, crates/viewer/src/main.rs
- **Verification:** `cargo generate-lockfile` produces 2715-line lockfile
- **Committed in:** `a202d88`, `33cc548`

**3. [Rule 1 - Bug] Regenerated Cargo.lock with compatible version**
- **Found during:** Task 3 (Cargo.lock generation)
- **Issue:** Initial Cargo.lock had version 4 which Rust 1.75 couldn't parse
- **Fix:** Removed lockfile and regenerated with `cargo generate-lockfile`
- **Files modified:** Cargo.lock
- **Verification:** `cargo metadata` succeeds, lockfile has version 3
- **Committed in:** `33cc548`

---

**Total deviations:** 3 auto-fixed (3 blocking issues)
**Impact on plan:** All auto-fixes necessary for correctness. Resolver change required due to Rust version constraint. Stub crates needed for lockfile generation.

## Issues Encountered
- System dependencies (libwayland-dev, libxkbcommon-dev) not available without sudo - full build verification deferred
- Cargo.lock version incompatibility resolved by regeneration

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Workspace structure complete and validated
- Ready for plan 01-02 (Server crate setup with Smithay)
- System dependencies will be needed for full build: `sudo apt-get install libwayland-dev libxkbcommon-dev`

---
*Phase: 01-project-foundation*
*Completed: 2026-03-10*

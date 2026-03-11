---
phase: 05-windows-viewer-foundation
plan: 01
subsystem: network
tags: [tokio, tcp, async, binary-protocol, rust]

requires:
  - phase: 04-tcp-frame-streaming
    provides: TCP server protocol specification (20-byte header + RGBA payload)
provides:
  - Async TCP client with Tokio runtime
  - Binary frame protocol parser (big-endian 20-byte header)
  - Frame streaming via mpsc channel to avoid blocking UI
  - Network error handling and logging
affects: [windows-viewer, frame-display, ui-thread]

tech-stack:
  added: [tokio async runtime, mpsc channels]
  patterns: [separate network thread, channel-based frame delivery, big-endian protocol]

key-files:
  created:
    - crates/viewer/src/lib.rs
    - crates/viewer/src/network/mod.rs
    - crates/viewer/src/network/protocol.rs
    - crates/viewer/src/network/client.rs
  modified: []

key-decisions:
  - "FrameHeader uses normal Rust struct (24 bytes) with SIZE constant (20) for wire format"
  - "Separate read_frame_from_stream helper for spawned tasks without client instance"
  - "100MB payload size sanity check to prevent unreasonable allocations"

requirements-completed: [VIEW-01]

duration: 12 min
completed: 2026-03-11
---

# Phase 5 Plan 1: TCP Client Foundation Summary

**Async TCP client with Tokio runtime, 20-byte big-endian frame protocol parser, and mpsc channel-based frame delivery to avoid blocking UI thread**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-11T10:54:35Z
- **Completed:** 2026-03-11T11:07:08Z
- **Tasks:** 3
- **Files modified:** 4

## Accomplishments
- Network module structure with Frame, FrameHeader, and NetworkError types
- Binary protocol parser with 20-byte big-endian header decoding (window_id, width, height, timestamp)
- Async TCP client using Tokio with configurable address and frame streaming via mpsc channel
- 15 unit tests covering protocol parsing, frame reading, and error handling

## Task Commits

Each task was committed atomically:

1. **Task 1-3: Network module, protocol parser, TCP client** - `8267914` (feat)

**Plan metadata:** (pending)

_Note: All 3 tasks completed in single commit as they are tightly coupled modules_

## Files Created/Modified
- `crates/viewer/src/lib.rs` - Library exports for network module
- `crates/viewer/src/network/mod.rs` - Module structure, Frame/FrameHeader types, NetworkError enum
- `crates/viewer/src/network/protocol.rs` - 20-byte big-endian header decode/encode, payload size calculation
- `crates/viewer/src/network/client.rs` - Async TcpClient with Tokio, read_frame, start_receiving with mpsc channel

## Decisions Made
- **FrameHeader struct layout:** Used normal Rust struct (24 bytes with padding) rather than `#[repr(packed)]` to avoid unaligned access UB. Wire format size tracked via `SIZE` constant (20 bytes).
- **Helper function for spawned tasks:** Created standalone `read_frame_from_stream()` to avoid requiring TcpClient instance in background tasks.
- **Payload size sanity check:** Added 100MB limit to prevent denial-of-service via malicious frame headers.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Created lib.rs to export network module**
- **Found during:** Task 1 (network module structure)
- **Issue:** crates/viewer/src/lib.rs did not exist, preventing module compilation
- **Fix:** Created lib.rs with network module export and public re-exports
- **Files modified:** crates/viewer/src/lib.rs
- **Verification:** cargo test passes, module accessible
- **Committed in:** 8267914 (Task 1-3 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Essential for module structure. No scope creep.

## Issues Encountered
- FrameHeader struct size mismatch: Rust alignment padding made struct 24 bytes instead of 20. Resolved by using SIZE constant for wire format and keeping struct as normal layout for internal use.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness

- TCP client foundation complete, ready for window integration
- Frame protocol parsing validated with 15 unit tests
- Network thread separation pattern established for UI responsiveness
- Ready for Phase 5 Plan 2 (window creation and display)

---
*Phase: 05-windows-viewer-foundation*
*Completed: 2026-03-11*
## Self-Check: PASSED

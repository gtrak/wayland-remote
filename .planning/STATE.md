# STATE: Wayland Remote

**Project:** Wayland Remote Compositor  
**Current Phase:** None — Roadmap just created  
**Last Updated:** 2025-03-10  
**Mode:** yolo (comprehensive depth)  

---

## Project Reference

### Core Value
Users can run Linux GUI applications remotely and interact with them as native windows on Windows, with full input support and acceptable performance.

### Architecture Summary
- **Linux Server:** Smithay 0.7.0-based headless compositor
- **Rendering:** PixmanRenderer for software/offscreen rendering
- **Protocol:** Custom binary TCP streaming (raw RGBA frames)
- **Windows Viewer:** Native Win32 application with GDI display
- **Stack:** Rust, Smithay, Tokio, winit, Win32 API

### Key Decisions
| Decision | Rationale | Status |
|----------|-----------|--------|
| Frame streaming vs protocol proxy | Simpler, more robust | Pending validation |
| Smithay over wlroots | Rust-native, mature | Pending validation |
| GDI over Direct3D | Easiest path for MVP | Pending validation |
| Raw RGBA over H264 | Focus on correctness first | Pending validation |
| TDD red-green-refactor | Ensures incremental progress | Pending validation |

---

## Current Position

### Phase Status
- **Active Phase:** None (roadmap creation complete)
- **Next Phase:** Phase 1 — Project Foundation
- **Phase Completion:** 0/8 phases

```
[░░░░░░░░░░░░░░░░░░] 0% Overall Progress
Phase: None (planning complete)
```

### Current Plan
- **Plan:** None
- **Status:** Not started
- **Blocking:** None

### Recent Activity
- [2025-03-10] Project initialized
- [2025-03-10] Research completed (6 critical pitfalls identified)
- [2025-03-10] Roadmap created (8 phases, 22 requirements mapped)

---

## Performance Metrics

| Metric | Target | Current | Status |
|--------|--------|---------|--------|
| Frame rate | 30fps @ 1080p | N/A | ⏳ Pending |
| Latency | <50ms | N/A | ⏳ Pending |
| Bandwidth | ~90MB/s (raw RGBA) | N/A | ⏳ Pending |
| Memory | TBD | N/A | ⏳ Pending |

---

## Accumulated Context

### Critical Pitfalls (from Research)
These must be addressed in their respective phases:

1. **Frame Callback Timing** (Phase 3) — Always respond to wl_surface.frame, throttle to network capacity
2. **Buffer Release Ordering** (Phase 3) — Release buffers only after full transmission
3. **XDG Configure/Ack** (Phase 7) — Track serials, wait for ack before applying geometry
4. **Input Event Serials** (Phase 8) — Maintain monotonic serial counter per-seat
5. **XKB Keymap** (Phase 8) — Use xkbcommon, send via FD, evdev scancode + 8
6. **Surface Roles** (Phase 7) — Use PopupManager, track parent-child relationships

### Technical Decisions
- **Smithay 0.7.0**: Pinned version, check changelog before updates
- **Tokio for async**: TCP streaming server
- **winit 0.30.x**: Windows window management
- **Security via SSH**: No built-in authentication (by design)

### Open Questions
1. How will raw RGBA bandwidth (~90MB/s for 1080p) perform on real LAN?
2. Which applications to test for toolkit compatibility (GTK, Qt, SDL)?
3. Windows DPI handling specifics for HiDPI displays?
4. Exact buffer synchronization timing for release-after-transmission?

### Known Risks
| Risk | Mitigation | Phase |
|------|------------|-------|
| Smithay API changes | Pin version 0.7.0, review changelog | All |
| Bandwidth saturation | Early testing, consider Phase 6 acceleration | 4 |
| Toolkit compatibility | Test with diverse apps (GTK, Qt, SDL) | 2, 7, 8 |
| Backpressure handling | Test with slow networks | 4 |

---

## Session Continuity

### Current Focus
**Phase 1: Project Foundation**
- Establish Rust workspace structure
- Configure server and viewer crates
- Set up dependencies and build system
- Initialize CI/CD pipeline

### Next Actions
1. `/gsd-plan-phase 1` — Create detailed plan for Phase 1
2. Review plan with user
3. Execute Phase 1 implementation

### State of the World
- Project definition: ✓ Complete
- Research: ✓ Complete (6 pitfalls documented)
- Requirements: ✓ Complete (22 v1 requirements)
- Roadmap: ✓ Complete (8 phases defined)
- Planning: ⏳ Ready to begin Phase 1 planning

---

## Blockers

None currently.

---

*State initialized: 2025-03-10*
*Next: Phase 1 planning*

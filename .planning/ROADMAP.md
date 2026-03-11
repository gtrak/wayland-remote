# ROADMAP: Wayland Remote

**Project:** Wayland Remote Compositor
**Core Value:** Users can run Linux GUI applications remotely and interact with them as native windows on Windows, with full input support and acceptable performance.
**Phases:** 8
**Depth:** Comprehensive
**Coverage:** 22/22 requirements mapped ✓

---

## Phases

- [ ] **Phase 1: Project Foundation** - Rust workspace, crates, dependencies, and build configuration
- [ ] **Phase 2: Wayland Core Protocol** - Headless compositor accepting Wayland clients and managing surfaces
- [x] **Phase 3: Headless Rendering** - Offscreen rendering pipeline with Pixman and RGBA capture (completed 2026-03-10)
- [ ] **Phase 4: TCP Frame Streaming** - Binary protocol server streaming frames to viewer
- [ ] **Phase 5: Windows Viewer Foundation** - Windows application with TCP client and basic display
- [x] **Phase 6: Surface-to-HWND Mapping** - Multi-window support with proper window lifecycle (completed 2026-03-11)
- [ ] **Phase 7: XDG Shell Window Management** - Desktop window management (maximize, minimize, close, popups)
- [ ] **Phase 8: Bidirectional Input** - Keyboard and mouse from Windows to Linux applications

---

## Phase Details

### Phase 1: Project Foundation
**Provenance:** [A] — Authoritative-driven
**DerivedFrom:** (project infrastructure - no specific requirement ID, but required for all)
**Forces:** Rust workspace structure needed before any code can be written
**Fails if removed:** No project exists; cannot compile, test, or deploy anything
**Goal:** Development environment ready with workspace structure, dependencies, and build configuration
**Depends on:** Nothing (first phase)
**Requirements:** Infrastructure foundation
**Success Criteria** (what must be TRUE):
  1. `cargo build` succeeds with workspace structure containing `server/` and `viewer/` crates
  2. Smithay 0.7.0, Tokio 1.40+, and winit 0.30.x dependencies resolve correctly
  3. CI/CD pipeline runs tests on commit
  4. Both crates compile without errors
**Plans:** 2/4 plans executed

**Plan List:**
- [ ] 01-01-PLAN.md — Virtual workspace root configuration
- [ ] 01-02-PLAN.md — Server crate setup with Smithay dependency
- [ ] 01-03-PLAN.md — Viewer crate setup with winit dependency
- [ ] 01-04-PLAN.md — CI/CD pipeline with GitHub Actions

### Phase 2: Wayland Core Protocol
**Provenance:** [A] — Authoritative-driven
**DerivedFrom:** WAYL-01, WAYL-02, WAYL-03
**Forces:** Wayland-01 requires compositor accepting client connections; applications need surfaces to render to
**Fails if removed:** No Wayland server exists; Linux applications cannot connect or create windows
**Goal:** Headless compositor accepting Wayland client connections and managing surfaces
**Depends on:** Phase 1
**Requirements:** WAYL-01, WAYL-02, WAYL-03
**Success Criteria** (what must be TRUE):
  1. Wayland client (like `weston-simple-egl` or custom test client) connects and creates surfaces without errors
  2. Applications can attach buffers and commit surface changes
  3. wl_compositor, wl_surface, and wl_seat globals are available
  4. Surface destruction releases resources without leaks
**Plans:** 3 plans created

**Plan List:**
- [ ] 02-01-PLAN.md — Core compositor setup (CompositorState, Display, event loop)
- [ ] 02-02-PLAN.md — Seat and output globals (wl_seat, wl_output, client management)
- [ ] 02-03-PLAN.md — Surface lifecycle and testing (buffer attach, commit, destruction)

### Phase 3: Headless Rendering
**Provenance:** [A] — Authoritative-driven
**DerivedFrom:** REND-01, REND-02, REND-03
**Forces:** REND-01 requires headless/offscreen rendering; surfaces need rendering before frames can be streamed
**Fails if removed:** No framebuffer to capture; cannot extract pixel data for streaming
**Goal:** Offscreen rendering to memory buffers with RGBA pixel extraction
**Depends on:** Phase 2
**Requirements:** REND-01, REND-02, REND-03
**Success Criteria** (what must be TRUE):
  1. Surfaces render to offscreen buffer without physical display attached
  2. RGBA pixel data can be extracted from rendered surfaces
  3. Buffer lifecycle properly managed (attach → render → release)
  4. Frame callbacks respond at appropriate rate to prevent application freezing
Plans:** 3 plans created
**Plan List:**
- [ ] 03-01-PLAN.md — PixmanRenderer setup and initialization
- [ ] 03-02-PLAN.md — Surface rendering to offscreen buffers
- [ ] 03-03-PLAN.md — RGBA extraction and frame callback management

### Phase 4: TCP Frame Streaming
**Provenance:** [A] — Authoritative-driven
**DerivedFrom:** STREAM-01, STREAM-02, STREAM-03, STREAM-04
**Forces:** STREAM-01 requires TCP server; frames need to reach Windows viewer over network
**Fails if removed:** No network connection to viewer; frames stay on Linux server
**Goal:** TCP server accepting viewer connections and streaming raw RGBA frames
**Depends on:** Phase 3
**Requirements:** STREAM-01, STREAM-02, STREAM-03, STREAM-04
**Success Criteria** (what must be TRUE):
  1. TCP server accepts connections on configurable port
  2. Binary frame protocol sends header (width, height, timestamp) followed by RGBA data
  3. Multiple surfaces tracked and streamed with unique window IDs
  4. Server handles disconnections gracefully without compositor crash
**Plans:** 3 plans created

**Plan List:**
- [x] 04-01-PLAN.md — Streaming module foundation with binary protocol
- [x] 04-02-PLAN.md — TCP server implementation with client handling
- [x] 04-03-PLAN.md — Multi-surface tracking with window IDs

### Phase 5: Windows Viewer Foundation
**Provenance:** [A] — Authoritative-driven
**DerivedFrom:** VIEW-01, VIEW-02
**Forces:** VIEW-01 requires Windows application; frames need display on Windows
**Fails if removed:** No way to view remote windows on Windows; project has no client-side
**Goal:** Windows application connecting to server and displaying frames in a window
**Depends on:** Phase 4 (for protocol definition, can develop in parallel once protocol spec is stable)
**Requirements:** VIEW-01, VIEW-02
**Success Criteria** (what must be TRUE):
  1. Windows application connects to Linux server via TCP
  2. Frame header parsed correctly (width, height, timestamp)
  3. RGBA frames displayed using GDI StretchDIBits
  4. At least one window visible with correct colors and dimensions
**Plans:** 3 plans created

**Plan List:**
- [x] 05-01-PLAN.md — TCP client foundation (network module, protocol parser, async client) — Complete 2026-03-11
- [ ] 05-02-PLAN.md — Window display with GDI rendering (winit + GDI StretchDIBits)
- [ ] 05-03-PLAN.md — Integration (main entry, channel wiring, end-to-end flow)

### Phase 6: Surface-to-HWND Mapping
**Provenance:** [A] — Authoritative-driven
**DerivedFrom:** VIEW-03, VIEW-04
**Forces:** VIEW-03 requires surface-to-HWND mapping; multiple windows need individual management
**Fails if removed:** Only single window supported; cannot handle multiple Wayland surfaces
**Goal:** Each Wayland surface maps to native Windows HWND with resize support
**Depends on:** Phase 5
**Requirements:** VIEW-03, VIEW-04
**Success Criteria** (what must be TRUE):
  1. Each Wayland surface creates corresponding Windows HWND
  2. Surface creation/destroy properly manages HWND lifecycle
  3. Window resizes handled (frame scaling via StretchDIBits)
  4. Multiple windows visible simultaneously with correct content
**Plans:** 4/4 plans complete

**Plan List:**
- [ ] 06-01-PLAN.md — WindowManager core with bidirectional HashMaps
- [ ] 06-02-PLAN.md — ViewerApp multi-window integration
- [ ] 06-03-PLAN.md — Window resize handling with aspect ratio preservation
- [ ] 06-04-PLAN.md — Window lifecycle management (close, cleanup, exit)

### Phase 7: XDG Shell Window Management
**Provenance:** [A] — Authoritative-driven
**DerivedFrom:** WM-01, WM-02, WM-03, WM-04
**Forces:** WM-01 requires XDG shell support; desktop applications need window management (maximize, minimize, close)
**Fails if removed:** Windows have no decoration states; cannot maximize, minimize, or close properly; popups fail
**Goal:** Desktop window management with XDG shell (configure/ack, states, popups)
**Depends on:** Phase 2, Phase 6
**Requirements:** WM-01, WM-02, WM-03, WM-04
**Success Criteria** (what must be TRUE):
  1. xdg_wm_base, xdg_surface, xdg_toplevel protocols supported
  2. Configure/acknowledge handshake implemented for resizes
  3. Window states (maximize, minimize, fullscreen, close) work correctly
  4. Popup windows (menus, tooltips) display in correct location relative to parent
**Plans:** TBD

### Phase 8: Bidirectional Input
**Provenance:** [A] — Authoritative-driven
**DerivedFrom:** INPUT-01, INPUT-02, INPUT-03, INPUT-04
**Forces:** INPUT-01 requires keyboard input; applications need interaction to be useful
**Fails if removed:** Windows are display-only; cannot click, type, or interact with applications
**Goal:** Keyboard and mouse input captured on Windows and injected into Wayland applications
**Depends on:** Phase 2, Phase 6
**Requirements:** INPUT-01, INPUT-02, INPUT-03, INPUT-04
**Success Criteria** (what must be TRUE):
  1. Keyboard input captured via Win32 and sent to server
  2. Mouse movement and button clicks captured and transmitted
  3. Server injects input events into Wayland input pipeline
  4. XKB keymap handling produces correct characters (respecting layout)
**Plans:** TBD

---

## Requirements Coverage

### Phase-to-Requirement Mapping

| Phase | Requirements | Count |
|-------|--------------|-------|
| Phase 1: Project Foundation | Infrastructure | - |
| Phase 2: Wayland Core Protocol | WAYL-01, WAYL-02, WAYL-03 | 3 |
| Complete    | 2026-03-10 | 3 |
| Phase 4: TCP Frame Streaming | STREAM-01, STREAM-02, STREAM-03, STREAM-04 | 4 |
| Phase 5: Windows Viewer Foundation | VIEW-01, VIEW-02 | 2 |
| Phase 6: Surface-to-HWND Mapping | VIEW-03, VIEW-04 | 2 |
| Phase 7: XDG Shell Window Management | WM-01, WM-02, WM-03, WM-04 | 4 |
| Phase 8: Bidirectional Input | INPUT-01, INPUT-02, INPUT-03, INPUT-04 | 4 |
| **Total** | | **22/22** |

### Coverage Validation

✓ **All 22 v1 requirements mapped**
✓ **No orphaned requirements**
✓ **No duplicate assignments** — each requirement assigned to exactly one phase

---

## Dependencies

```
Phase 1 → Phase 2 → Phase 3 → Phase 4 → Phase 5 → Phase 6
                                          ↓           ↓
                                    Phase 7 ←──── Phase 8
                                          ↑
                                    (both need Wayland core)
```

**Dependency Notes:**
- Phase 5 (Windows Viewer) can begin once Phase 4 protocol spec is stable (parallel development)
- Phase 6 requires Phase 5 (HWND management builds on viewer foundation)
- Phase 7 requires Phase 2 (XDG shell builds on Wayland core) and Phase 6 (window states need HWNDs)
- Phase 8 requires Phase 2 (input injection needs Wayland core) and Phase 6 (input targets need HWNDs)

---

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Project Foundation | 2/4 | In Progress|  |
| 2. Wayland Core Protocol | 0/TBD | Not started | - |
| 3. Headless Rendering | 3/3 | ✓ Complete | 2026-03-10 |
| 4. TCP Frame Streaming | 3/3 | Planned | - |
| 5. Windows Viewer Foundation | 0/TBD | Not started | - |
| 6. Surface-to-HWND Mapping | 0/TBD | Complete    | 2026-03-11 |
| 7. XDG Shell Window Management | 0/TBD | Not started | - |
| 8. Bidirectional Input | 0/TBD | Not started | - |

---

## Research Flags

Based on research summary, these phases need attention:

| Phase | Flag | Action |
|-------|------|--------|
| Phase 4 (TCP Streaming) | ⚠️ Needs validation | Custom protocol design needs testing with slow networks to verify backpressure handling |
| Phase 8 (Input Handling) | ⚠️ Needs research | XKB integration has subtleties; serial tracking requires careful testing |

Standard patterns sufficient for:
- Phase 2: Well-documented Smithay patterns; Anvil reference available
- Phase 3: PixmanRenderer is straightforward
- Phase 5: Win32 GDI patterns are mature
- Phase 6: HWND management is standard
- Phase 7: XDG shell patterns documented in Smithay

---

## Deferred to v2

These requirements are authoritative but deferred:

| ID | Requirement | Reason |
|----|-------------|--------|
| PERF-01 | Damage tracking | Post-MVP bandwidth optimization |
| PERF-02 | H264 video encoding | Post-MVP bandwidth optimization |
| PERF-03 | Hardware acceleration (dmabuf) | Post-MVP performance |
| ADV-01 | Clipboard synchronization | Post-MVP feature |
| ADV-02 | Session persistence | Post-MVP feature |
| ADV-03 | XWayland support | Post-MVP feature |
| ADV-04 | Multi-monitor support | Post-MVP feature |

---

*Roadmap created: 2025-03-10*
*Depth: Comprehensive | Phases: 8 | Requirements: 22/22 mapped*

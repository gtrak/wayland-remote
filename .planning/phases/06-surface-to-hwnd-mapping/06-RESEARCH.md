# Phase 6 Research: Surface-to-HWND Mapping

**Phase:** 06 — Surface-to-HWND Mapping  
**Requirements:** VIEW-03, VIEW-04  
**Research Date:** 2026-03-11  
**Status:** Ready for Planning

---

## 1. Executive Summary

Phase 6 implements the critical bridge between the server's multi-surface tracking (completed in Phase 4) and the Windows viewer's display capabilities (completed in Phase 5). Each Wayland surface tracked by the server's `SurfaceTracker` must create a corresponding Windows HWND on the viewer side, with proper lifecycle management and resize support.

**Current State:**
- Server: `SurfaceTracker` provides unique window IDs (AtomicU32) with bidirectional ObjectId <-> window_id mappings
- Protocol: Frame header includes `window_id: u32` to identify which surface the frame belongs to
- Viewer: Single-window implementation using winit 0.30.x ApplicationHandler with GDI rendering

**Phase 6 Must Achieve:**
1. **VIEW-03**: Each Wayland surface creates corresponding Windows HWND
2. **VIEW-04**: Window resizes handled (frame scaling via StretchDIBits)

---

## 2. Technical Deep Dive

### 2.1 winit 0.30.x Multi-Window Architecture

winit 0.30.x uses the `ApplicationHandler` trait pattern where the application struct implements event callbacks. Key characteristics for multi-window support:

**Window Creation:**
- Windows are created via `event_loop.create_window(window_attributes)`
- Each call returns a `Window` instance with a unique `WindowId`
- Multiple windows can exist simultaneously
- Windows are independent but share the same event loop

**Window Identification:**
- `WindowId` is a unique identifier for each window (not the HWND directly)
- `window.id()` returns the `WindowId`
- `WindowId` can be converted to/from raw representation for storage
- `RawWindowHandle` provides platform-specific handles (HWND on Windows)

**Event Routing:**
- `window_event()` callback receives `(WindowId, WindowEvent)`
- Events are automatically routed to the correct window based on WindowId
- All windows share the same ApplicationHandler instance

### 2.2 HWND Access Patterns

**From winit Window to HWND:**
```rust
// Method 1: via raw-window-handle trait
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
let handle = window.window_handle().unwrap();
if let RawWindowHandle::Win32(handle) = handle.as_raw() {
    let hwnd = handle.hwnd.get(); // HWND as isize
}

// Method 2: via WindowId (current approach in app.rs line 109)
let hwnd = window.id().as_raw() as *mut c_void;
```

**Current Implementation Analysis:**
- `app.rs:109` uses `window.id().as_raw() as *mut c_void` to get HWND
- This works because on Windows, winit's WindowId IS the HWND value
- However, this is platform-specific and relies on implementation details
- For robustness, should use `HasWindowHandle` trait from raw-window-handle

### 2.3 Frame Routing Architecture

**Protocol-Level Window IDs:**
- Server assigns window_id via `SurfaceTracker::allocate_window_id()` (starts at 1)
- Frame header includes `window_id: u32` (20-byte header, big-endian)
- Window ID 0 is reserved/invalid (SurfaceTracker starts at 1)

**Viewer-Level Window Tracking:**
Need two-way mapping to route frames:
1. `window_id (u32)` -> `WindowId` (for routing incoming frames)
2. `WindowId` -> `DisplayWindow` (for event handling)

**Lifecycle Events:**
- Surface creation (server) -> First frame with new window_id arrives -> Create window
- Surface destruction (server) -> Server sends special "destroy" message OR connection drops frames -> Destroy window
- Window close (viewer) -> Should notify server (future Phase 7/8 feature)

### 2.4 Resize Handling

**Current Implementation (05-02):**
- `DisplayWindow::submit_frame()` resizes window if frame dimensions differ by >10%
- Uses `window.set_inner_size(PhysicalSize::new(width, height))`
- GDI renderer uses `StretchDIBits` with aspect-ratio-preserving scaling

**Multi-Window Resize Considerations:**
- Each window resizes independently
- Resize threshold (10%) prevents flickering from minor dimension changes
- Window resizes trigger `WindowEvent::Resized` -> redraw
- Frame scaling already handles aspect ratio preservation in `gdi.rs:243-253`

---

## 3. Implementation Options

### Option A: Dynamic Window Creation (Recommended)

**Approach:** Create windows on-demand when first frame for a new window_id arrives

**Pros:**
- Simple mapping: window_id directly drives window creation
- No need for special "create window" protocol messages
- Natural lifecycle: window exists only when surface has content
- Matches Wayland philosophy (surfaces are content-bearing)

**Cons:**
- Window might appear with delay (after first frame arrives)
- Window position not controllable (always appears at default location)
- No way to set window title from server (could add to protocol later)

**Implementation:**
```rust
// In frame processing:
if let Some(frame) = rx.recv().await {
    let window_id = frame.header.window_id;
    if !windows.contains_key(window_id) {
        // Create new window
        let window = create_window(window_id, frame.header.width, frame.header.height);
        windows.insert(window_id, window);
    }
    windows[window_id].submit_frame(&frame);
}
```

### Option B: Explicit Window Management Protocol

**Approach:** Server sends explicit "create window" / "destroy window" messages

**Pros:**
- Viewer knows about windows before first frame
- Can set window properties (title, position) at creation
- Cleaner separation of concerns

**Cons:**
- Requires protocol changes (new message types)
- More complex server-side logic
- Needs synchronization between Wayland surface events and TCP messages

**Implementation:**
Would require extending protocol with:
- `WindowCreate { window_id, width, height, title }`
- `WindowDestroy { window_id }`
- `WindowConfigure { window_id, width, height }`

### Option C: Hybrid with Metadata in Frame Header

**Approach:** Extend frame header with flags indicating surface state

**Pros:**
- Minimal protocol changes
- Backward compatible
- Can signal "first frame" or "last frame" for lifecycle

**Cons:**
- Header size increases
- More complex frame parsing logic
- Still need heuristics for window destruction

**Implementation:**
Extend header to 24 bytes:
- bytes 20-23: flags (bit 0 = first frame, bit 1 = last frame)

### Recommendation

**Use Option A (Dynamic Window Creation)** for Phase 6 because:
1. Already have window_id in frame header (no protocol changes)
2. Simplest implementation with clear lifecycle
3. Matches current architecture (frames drive display)
4. Can enhance with Option B/C later for window titles/position

---

## 4. Recommended Approach

### 4.1 Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        ViewerApp                                 │
│  ┌─────────────────┐     ┌─────────────────┐                   │
│  │   HashMap<u32,  │     │   HashMap<       │                   │
│  │   DisplayWindow> │     │   WindowId, u32> │                  │
│  │   (window_id -> │     │   (reverse lookup)│                  │
│  │    window)       │     │                  │                   │
│  └────────┬────────┘     └─────────────────┘                   │
│           │                                                     │
│  ┌────────▼─────────────────────────────────┐                   │
│  │    ApplicationHandler Impl               │                   │
│  │  - resumed(): no-op (windows created     │                   │
│  │               dynamically)               │                   │
│  │  - window_event(): route to specific     │                   │
│  │                    DisplayWindow         │                   │
│  │  - process_frames(): create/update       │                   │
│  │                       windows per frame  │                   │
│  └──────────────────────────────────────────┘                   │
└─────────────────────────────────────────────────────────────────┘
```

### 4.2 Data Structures

```rust
/// Maps server window_id to viewer DisplayWindow
pub struct WindowManager {
    /// window_id (from server) -> DisplayWindow
    windows: HashMap<u32, DisplayWindow>,
    /// WindowId (winit) -> window_id (reverse lookup for events)
    window_to_id: HashMap<WindowId, u32>,
    /// Next window position offset (cascade new windows)
    next_position: (i32, i32),
}

impl WindowManager {
    pub fn create_window(&mut self, window_id: u32, width: u32, height: u32) -> WindowId;
    pub fn destroy_window(&mut self, window_id: u32);
    pub fn get_window(&self, window_id: u32) -> Option<&DisplayWindow>;
    pub fn get_window_id(&self, winit_id: WindowId) -> Option<u32>;
}
```

### 4.3 Frame Processing Flow

```rust
fn process_frames(&mut self) {
    if let Some(ref mut rx) = self.frame_rx {
        while let Ok(frame) = rx.try_recv() {
            let window_id = frame.header.window_id;
            
            // Get or create window
            match self.window_manager.get_window_mut(window_id) {
                Some(window) => window.submit_frame(&frame),
                None => {
                    // First frame for this window_id - create window
                    let window = self.window_manager.create_window(
                        window_id,
                        frame.header.width,
                        frame.header.height
                    );
                    window.submit_frame(&frame);
                }
            }
        }
    }
}
```

### 4.4 Window Lifecycle

**Creation:**
1. Server assigns window_id via `SurfaceTracker::allocate_window_id()`
2. Server streams first frame with window_id in header
3. Viewer receives frame, creates `DisplayWindow` with that ID
4. Window appears on screen with first frame content

**Destruction (Current - Simple):**
1. Wayland surface destroyed on server
2. Server stops sending frames for that window_id
3. Viewer: Optional timeout-based cleanup (Phase 6 v1)
4. Or: User closes window manually (viewer-side only)

**Destruction (Future - Phase 7):**
- Server sends explicit "surface destroyed" message
- Viewer receives message, destroys corresponding window
- Requires protocol extension

**Destruction (Future - Phase 8):**
- User closes window on Windows side
- Viewer sends "window closed" to server
- Server destroys Wayland surface
- Bidirectional lifecycle

---

## 5. Critical Implementation Details

### 5.1 Window Position Cascading

**Problem:** All windows would stack at (0,0) by default

**Solution:** Implement cascading window positions
```rust
const CASCADE_OFFSET: i32 = 30;

fn get_next_window_position(&mut self) -> (i32, i32) {
    let (x, y) = self.next_position;
    self.next_position = (
        (x + CASCADE_OFFSET) % 300, // Wrap after 300px
        (y + CASCADE_OFFSET) % 300,
    );
    (x, y)
}
```

### 5.2 Window Title Strategy

**Current:** Use generic title like "Wayland Remote - Window {window_id}"

**Future Enhancement:** Add title field to frame header or protocol extension

**Implementation:**
```rust
let title = format!("Wayland Remote - Window {}", window_id);
let window_attrs = Window::default_attributes()
    .with_title(&title)
    // ... other attrs
```

### 5.3 Event Loop Integration

**Challenge:** winit 0.30 ApplicationHandler receives events for ALL windows

**Solution:** Route events using reverse mapping
```rust
fn window_event(&mut self, event_loop: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
    // Map WindowId to server window_id
    if let Some(server_id) = self.window_manager.get_window_id(window_id) {
        match event {
            WindowEvent::CloseRequested => {
                self.window_manager.destroy_window(server_id);
                if self.window_manager.is_empty() {
                    event_loop.exit();
                }
            }
            WindowEvent::Resized(size) => {
                if let Some(window) = self.window_manager.get_window(server_id) {
                    window.handle_resize(size);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(window) = self.window_manager.get_window(server_id) {
                    window.on_paint();
                }
            }
            _ => {}
        }
    }
}
```

### 5.4 Thread Safety Considerations

**Current Architecture:**
- Network thread: Receives frames from TCP, sends via mpsc
- Main/UI thread: Runs winit event loop, owns all Windows

**Phase 6 Changes:**
- Window creation/destruction must happen on main thread
- mpsc channel already ensures this (network sends, main receives)
- No additional synchronization needed

**GDI Context:**
- Each `DisplayWindow` has its own GDI resources (HDC, bitmaps)
- GDI is not thread-safe, but each window is only accessed from main thread
- Current implementation already handles this correctly

### 5.5 Memory Management

**Window Cleanup:**
```rust
impl Drop for WindowManager {
    fn drop(&mut self) {
        // DisplayWindow::Drop already cleans up GDI resources
        // HashMap drop will call Drop for each DisplayWindow
    }
}
```

**Frame Buffering:**
- Keep current mpsc buffer size of 10 frames
- Each window has its own GDIRenderer with double buffering
- Memory scales linearly with window count: O(n) where n = number of surfaces

---

## 6. Common Pitfalls

### 6.1 WindowId vs window_id Confusion

**Pitfall:** Mixing up winit's `WindowId` (event identifier) with server's `window_id` (u32)

**Mitigation:** Clear naming convention:
- `winit_id` or `window_id_winit` for winit WindowId
- `server_id` or `window_id_server` for server u32
- `window_id` only when context is clear

### 6.2 HWND Lifecycle

**Pitfall:** Accessing HWND after window is destroyed

**Mitigation:** 
- Always check `window.is_some()` before accessing
- Use `if let Some(window) = windows.get(&id)` pattern
- Drop `DisplayWindow` before removing from HashMap

### 6.3 Event Loop Blocking

**Pitfall:** Creating windows synchronously in event handlers can block

**Mitigation:**
- Window creation is fast in winit, but monitor with tracing
- If slow, defer to `new_events` or use `proxy.send_event`
- Current approach (create in `process_frames` called from `new_events`) is good

### 6.4 Frame Ordering

**Pitfall:** Frames for new window_id arriving before window is created

**Mitigation:**
- `HashMap::entry().or_insert_with()` pattern ensures atomic get-or-create
- No frames are dropped; first frame triggers creation

### 6.5 Window Focus/Activation

**Pitfall:** New windows stealing focus from each other

**Mitigation:**
- Use `with_active(false)` or platform-specific window level
- Test with multiple rapid window creations
- May need to queue window creations if too rapid

### 6.6 Resize Feedback Loop

**Pitfall:** Window resize triggers frame resize triggers window resize

**Current Mitigation:**
- 10% threshold in `DisplayWindow::submit_frame()` prevents feedback loop
- Resize only if dimensions changed significantly
- This should be preserved in multi-window implementation

---

## 7. Validation Architecture

### 7.1 Unit Tests

**WindowManager Tests:**
```rust
#[test]
fn test_window_lifecycle() {
    let mut wm = WindowManager::new(&event_loop);
    
    // Create window
    let winit_id = wm.create_window(1, 800, 600);
    assert!(wm.get_window(1).is_some());
    assert_eq!(wm.get_window_id(winit_id), Some(1));
    
    // Destroy window
    wm.destroy_window(1);
    assert!(wm.get_window(1).is_none());
}

#[test]
fn test_frame_routing() {
    let mut wm = WindowManager::new(&event_loop);
    wm.create_window(1, 800, 600);
    wm.create_window(2, 400, 300);
    
    // Frame for window 1
    let frame1 = Frame { header: FrameHeader { window_id: 1, ... }, ... };
    wm.get_window(1).unwrap().submit_frame(&frame1);
    
    // Verify window 2 didn't receive it
    assert_ne!(wm.get_window(2).unwrap().frame_dimensions(), (800, 600));
}
```

**Integration Tests:**
- Mock server sending frames for multiple window_ids
- Verify multiple HWNDs are created
- Verify each window shows correct content
- Verify closing one window doesn't affect others

### 7.2 Manual Testing Scenarios

**Scenario 1: Multiple Applications**
```
1. Start server
2. Connect viewer
3. Run `weston-simple-egl` on server (creates window 1)
4. Run `weston-simple-damage` on server (creates window 2)
5. Verify: Two windows appear on Windows
6. Verify: Each shows different content
7. Close window 1 (viewer side)
8. Verify: window 2 still shows content
```

**Scenario 2: Resize Handling**
```
1. Open application with resizable window
2. Resize window on Windows side (drag corner)
3. Verify: Window resizes smoothly
4. Verify: Content scales with aspect ratio preserved
5. Verify: Frame continues to update
```

**Scenario 3: Rapid Creation/Destruction**
```
1. Run script that creates/destroys windows rapidly
2. Verify: No memory leaks
3. Verify: No orphaned HWNDs (check with Spy++)
4. Verify: Viewer remains responsive
```

### 7.3 Performance Metrics

**Metrics to Track:**
- Memory per window: Should be ~4 bytes/pixel + overhead
- Creation latency: <100ms from first frame to visible window
- Resize latency: <50ms from resize event to redraw
- Frame routing overhead: O(1) HashMap lookup

**Monitoring:**
```rust
tracing::info!(
    window_id = window_id,
    duration_ms = start.elapsed().as_millis(),
    "Window created"
);
```

---

## 8. Dependencies and Prerequisites

### 8.1 Existing Dependencies (No Changes Required)

**Verified Available:**
- winit 0.30.x (confirmed in Cargo.toml)
- winapi 0.3 (confirmed with wingdi, windef, minwindef features)
- raw-window-handle (workspace dependency, available)
- tokio (workspace dependency)
- tracing (workspace dependency)

### 8.2 Rust Standard Library Features

**Required:**
- `std::collections::HashMap` (already used)
- `std::sync::mpsc` (already used)
- No additional crates needed

### 8.3 Prerequisites from Prior Phases

**Must Be Complete:**
- Phase 4: SurfaceTracker with window_id allocation ✓
- Phase 5: Single-window viewer with GDI rendering ✓
- Protocol: Frame header with window_id field ✓

**Assumptions:**
- Server sends window_id in frame header (confirmed in protocol.rs)
- window_id is stable for surface lifetime (guaranteed by SurfaceTracker)
- window_id starts at 1 (confirmed in surface.rs)

### 8.4 Platform Requirements

**Windows-Specific:**
- Win32 GDI (already used)
- HWND manipulation (already used via winit)
- No new Win32 APIs needed

---

## 9. File Structure

### 9.1 New Files to Create

```
crates/viewer/src/
├── window/
│   ├── mod.rs          # WindowManager and exports
│   └── manager.rs      # WindowManager implementation
└── app.rs              # Modify for multi-window support
```

### 9.2 Files to Modify

**crates/viewer/src/app.rs:**
- Replace `Option<DisplayWindow>` with `WindowManager`
- Update `process_frames()` to route to correct window
- Update `window_event()` to handle multiple windows

**crates/viewer/src/display/mod.rs:**
- Add `WindowManager` to public exports

**crates/viewer/src/lib.rs:**
- Add `window` module

### 9.3 No Changes Required

- `crates/viewer/src/display/window.rs` - DisplayWindow is already multi-window ready
- `crates/viewer/src/display/gdi.rs` - GDI renderer per-window is correct
- `crates/viewer/src/network/*` - Protocol already supports multi-window
- `Cargo.toml` - No new dependencies

---

## 10. Integration Points

### 10.1 Server Integration (Phase 4)

**Interface:** Frame protocol with window_id in header
**Data Flow:**
```
Server SurfaceTracker (ObjectId -> window_id)
    ↓ (TCP)
Viewer FrameHeader (window_id)
    ↓ (HashMap lookup)
WindowManager (window_id -> DisplayWindow)
    ↓
GDI Renderer (StretchDIBits)
```

**Contract:**
- Server guarantees unique, stable window_id per surface
- window_id 0 is invalid/unused
- window_id is u32 in big-endian in frame header

### 10.2 Phase 5 Integration

**Building On:**
- `ViewerApp` ApplicationHandler pattern (keep structure)
- `DisplayWindow` with GDI rendering (reuse as-is)
- mpsc channel frame streaming (keep as-is)
- Network thread spawning (keep as-is)

**Modifications:**
- Change `display_window: Option<DisplayWindow>` to `window_manager: WindowManager`
- Route events via reverse mapping (WindowId -> window_id)
- Create windows dynamically instead of in `resumed()`

### 10.3 Phase 7/8 Preparation

**Future Integration Points:**
- Window close events (Phase 7: send to server)
- Window titles (Phase 7: protocol extension)
- Input routing (Phase 8: needs WindowId -> window_id mapping)
- Window states (Phase 7: maximize/minimize/fullscreen)

**Preparations in Phase 6:**
- Keep reverse mapping (WindowId -> window_id) for input routing
- Design WindowManager to support window metadata (title, state)
- Ensure clean destruction for bidirectional lifecycle

### 10.4 Protocol Stability

**Current Protocol (No Changes):**
```rust
// FrameHeader (20 bytes, big-endian)
pub struct FrameHeader {
    pub window_id: u32,  // Already present
    pub width: u32,
    pub height: u32,
    pub timestamp: u64,
}
```

**Future Extensions (Phase 7+):**
- May need WindowCreate/WindowDestroy/WindowConfigure messages
- Could extend header with flags
- Phase 6 should not block these extensions

---

## 11. Research Conclusions

### 11.1 Key Findings

1. **winit 0.30.x Multi-Window is Straightforward:**
   - Multiple `Window` objects can coexist
   - `ApplicationHandler::window_event()` routes by `WindowId`
   - No special configuration needed beyond creating multiple windows

2. **window_id is the Natural Key:**
   - Server already provides stable window_id via SurfaceTracker
   - Frame header already includes window_id
   - HashMap<u32, DisplayWindow> is the right abstraction

3. **HWND Access is Platform-Specific but Solved:**
   - Current approach (window.id().as_raw()) works on Windows
   - raw-window-handle trait provides cleaner cross-platform API
   - No changes needed to existing DisplayWindow GDI code

4. **Dynamic Window Creation is the Right Approach:**
   - No protocol changes required
   - Simple mental model: frames drive window lifecycle
   - Can enhance with explicit messages later (Phase 7+)

5. **Existing Code is Well-Positioned:**
   - DisplayWindow and GDI renderer already support multi-window
   - mpsc channel architecture naturally supports routing
   - Just need WindowManager to coordinate multiple windows

### 11.2 Risk Assessment

| Risk | Likelihood | Impact | Mitigation |
|------|-----------|--------|------------|
| Window creation performance | Low | Medium | Test with rapid creation, optimize if needed |
| Memory usage with many windows | Medium | Low | Monitor, implement LRU or limit if needed |
| Window focus issues | Medium | Low | Use proper window levels, test with focus changes |
| Resize feedback loop | Low | High | Preserve 10% threshold, test thoroughly |
| HWND lifecycle bugs | Low | High | Use RAII, test destruction scenarios |

**Overall Risk: LOW**

All technical challenges have straightforward solutions. The primary work is refactoring the single-window application structure to support multiple windows, which is a well-understood pattern in winit.

### 11.3 Recommended Next Steps

1. **Create 06-01-PLAN.md** with specific tasks for WindowManager implementation
2. **Start with window/manager.rs** implementing WindowManager struct
3. **Refactor app.rs** to use WindowManager instead of Option<DisplayWindow>
4. **Add cascading window positions** for UX
5. **Write comprehensive tests** for window lifecycle and routing
6. **Manual test** with multiple Wayland applications

**Estimated Effort:** 3-4 plans (similar complexity to Phase 5)
**Estimated Duration:** 30-45 minutes per plan
**Success Criteria:** VIEW-03 and VIEW-04 satisfied

---

*Research Complete: Phase 6 is Ready for Planning*  
*Next Action: Create 06-01-PLAN.md with implementation tasks*

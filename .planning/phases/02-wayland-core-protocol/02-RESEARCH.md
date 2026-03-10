# Phase 2: Wayland Core Protocol - Research

**Researched:** 2026-03-10
**Domain:** Wayland compositor development with Smithay 0.7.0
**Confidence:** HIGH

## Summary

Phase 2 requires implementing a headless Wayland compositor that accepts client connections and manages surfaces. Based on research of Smithay 0.7.0, the standard approach involves:

1. **Core Setup**: Initialize `CompositorState`, `SeatState`, and `OutputManagerState` to provide `wl_compositor`, `wl_seat`, and `wl_output` globals
2. **Socket Listener**: Use `ListeningSocketSource` from `smithay::wayland::socket` to accept client connections
3. **Event Loop**: Integrate with `calloop` for event-driven architecture (required by Smithay design)
4. **Surface Management**: Smithay automatically handles `wl_surface` lifecycle, buffer attachment, and commit handling via the `CompositorHandler` trait
5. **Headless Operation**: No physical display required - outputs are virtual/advertised via `Output::create_global()`

**Primary recommendation:** Follow the Smallvil pattern (minimal compositor) for Phase 2 foundation, then expand with Anvil patterns for production features. Use Smithay's delegate macros to avoid boilerplate.

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| WAYL-01 | Compositor accepts Wayland client connections and handles wl_compositor, wl_surface, wl_seat, wl_output protocols | `CompositorState::new()` for wl_compositor, `SeatState::new_wl_seat()` for wl_seat, `Output::create_global()` for wl_output |
| WAYL-02 | Applications can create surfaces, attach buffers, and commit changes | `CompositorHandler::commit()` callback handles commits; `with_states()` accesses surface data; `SurfaceAttributes` contains buffer info |
| WAYL-03 | Surface destruction and cleanup is handled properly | Smithay automatically invokes destruction hooks; implement `add_destruction_hook()` for custom cleanup |

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| smithay | 0.7.0 | Wayland compositor framework | Official Rust Wayland compositor library; used by COSMIC, Niri, and other production compositors |
| wayland-server | 0.31.9 | Wayland protocol server implementation | Smithay dependency; handles wire protocol |
| calloop | 0.14.0 | Event loop framework | Required by Smithay architecture; callback-oriented event handling |
| wayland-protocols | 0.32.8 | Wayland protocol definitions | Standard protocol definitions for wl_compositor, wl_surface, wl_seat, wl_output |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio | 1.40+ | Async runtime (project choice) | For TCP streaming in Phase 4; Wayland core uses calloop |
| tracing | 0.1.37+ | Logging | Smithay uses tracing extensively; required for debug output |
| xkbcommon | 0.8.0 | Keyboard handling | For wl_seat keyboard support (Phase 8) |
| anyhow | 1.0+ | Error handling | Project standard for error propagation |

### Smithay Features Required
For Phase 2 (minimal headless compositor):
- `wayland_frontend` (required for all Wayland protocol support)
- `backend_wlcs` (optional - for testing)

For Phase 3+ (rendering):
- `backend_pixman` or `backend_gles` for renderers

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Smithay | wlroots + Rust bindings | Smithay is Rust-native, better type safety; wlroots requires unsafe FFI |
| calloop | tokio | Smithay is built around calloop; tokio integration possible but complex |
| Headless | Hardware-accelerated | Headless chosen for remote deployment; hardware requires DRM access |

**Installation:**
```toml
# Cargo.toml
[dependencies]
smithay = { version = "0.7.0", features = ["wayland_frontend"] }
wayland-server = "0.31.9"
calloop = "0.14.0"
tracing = "0.1"
```

## Architecture Patterns

### Recommended Project Structure
```
crates/server/src/
├── main.rs              # Entry point, event loop setup
├── state.rs             # CompositorState struct implementing all Smithay handlers
├── handlers/
│   ├── compositor.rs    # CompositorHandler implementation
│   ├── seat.rs          # SeatHandler implementation (input)
│   └── output.rs        # Output/virtual display management
└── client.rs            # Per-client state (ClientData trait)
```

### Pattern 1: The Smallvil Pattern (Minimal Compositor)
**What:** Minimal viable compositor following Smithay's smallvil example
**When to use:** Phase 2 foundation; getting basic protocol support working
**Key components:**
- Single `State` struct holding all Smithay states
- `CompositorState::new::<State>()` in constructor
- `ListeningSocketSource::new_auto()` for socket creation
- `Display` integrated with `calloop` event loop

**Example:**
```rust
// Source: https://github.com/Smithay/smithay/blob/master/smallvil/src/state.rs
use smithay::wayland::{
    compositor::{CompositorClientState, CompositorState},
    output::OutputManagerState,
    seat::{Seat, SeatState},
    shm::ShmState,
    socket::ListeningSocketSource,
};
use smithay::reexports::{
    calloop::{EventLoop, generic::Generic, Interest, Mode, PostAction},
    wayland_server::{Display, protocol::wl_surface::WlSurface},
};

pub struct ServerState {
    pub display_handle: DisplayHandle,
    pub compositor_state: CompositorState,
    pub seat_state: SeatState<ServerState>,
    pub output_manager_state: OutputManagerState,
    pub shm_state: ShmState,
    pub seat: Seat<ServerState>,
    pub socket_name: OsString,
}

impl ServerState {
    pub fn new(event_loop: &mut EventLoop<Self>, display: Display<Self>) -> Self {
        let dh = display.handle();
        
        // Initialize core protocols
        let compositor_state = CompositorState::new::<Self>(&dh);
        let seat_state = SeatState::new();
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        
        // Create seat (required for clients to receive input focus)
        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "wayland-remote-seat");
        seat.add_keyboard(Default::default(), 200, 25).unwrap();
        seat.add_pointer();
        
        // Setup listening socket
        let listening_socket = ListeningSocketSource::new_auto().unwrap();
        let socket_name = listening_socket.socket_name().to_os_string();
        
        event_loop.handle()
            .insert_source(listening_socket, |client_stream, _, state| {
                state.display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                    .unwrap();
            })
            .expect("Failed to init wayland socket source");
        
        // Add display to event loop
        event_loop.handle()
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    unsafe { display.get_mut().dispatch_clients(state).unwrap(); }
                    Ok(PostAction::Continue)
                },
            )
            .unwrap();
        
        Self {
            display_handle: dh,
            compositor_state,
            seat_state,
            output_manager_state,
            shm_state,
            seat,
            socket_name,
        }
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}
```

### Pattern 2: CompositorHandler Implementation
**What:** Handle surface commits and lifecycle
**When to use:** Required for Phase 2; called on every buffer commit
**Key methods:**
- `commit()`: Called when surface state changes (buffer attached, etc.)
- `compositor_state()`: Returns mutable reference to `CompositorState`
- `client_compositor_state()`: Returns per-client compositor state

**Example:**
```rust
// Source: https://docs.rs/smithay/0.7.0/smithay/wayland/compositor/
use smithay::wayland::compositor::{
    CompositorHandler, CompositorState, CompositorClientState, 
    with_states, SurfaceData
};
use smithay::delegate_compositor;

impl CompositorHandler for ServerState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }
    
    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }
    
    fn commit(&mut self, surface: &WlSurface) {
        // Called on every surface commit
        // Access surface state with with_states()
        with_states(surface, |surface_data| {
            let attrs = surface_data.cached_state.current::<SurfaceAttributes>();
            if let Some(buffer_assignment) = &attrs.buffer {
                // Buffer was attached - handle in Phase 3
            }
        });
    }
}

delegate_compositor!(ServerState);
```

### Pattern 3: Virtual Output Creation
**What:** Advertise a virtual output for clients
**When to use:** Phase 2 - clients need wl_output to know where to render
**Key points:**
- Create `Output` with `Output::new()`
- Call `create_global()` to advertise to clients
- Set modes, scale, and position

**Example:**
```rust
// Source: https://docs.rs/smithay/0.7.0/smithay/wayland/output/
use smithay::output::{Output, PhysicalProperties, Mode, Subpixel, Scale};
use smithay::utils::Transform;

fn create_virtual_output(display_handle: &DisplayHandle) -> Output {
    let output = Output::new(
        "virtual-output-0".into(),
        PhysicalProperties {
            size: (0, 0).into(),  // Headless - no physical size
            subpixel: Subpixel::Unknown,
            make: "Wayland Remote".into(),
            model: "Virtual".into(),
        },
    );
    
    // Advertise to clients
    let _global = output.create_global::<ServerState>(display_handle);
    
    // Configure initial state
    output.change_current_state(
        Some(Mode { size: (1920, 1080).into(), refresh: 60000 }),
        Some(Transform::Normal),
        Some(Scale::Integer(1)),
        Some((0, 0).into()),
    );
    
    output.set_preferred(Mode { size: (1920, 1080).into(), refresh: 60000 });
    
    output
}
```

### Anti-Patterns to Avoid
- **Directly implementing Dispatch traits**: Use Smithay's `delegate_*!` macros instead
- **Manual socket management**: Use `ListeningSocketSource` for proper integration
- **Storing surface references**: Use `with_states()` to access surface data safely
- **Ignoring CompositorClientState**: Required for per-client state management

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Wayland protocol parsing | Custom parser | `wayland-server` crate | Wire protocol is complex; official crate handles object IDs, message marshaling, etc. |
| Surface state tracking | Manual HashMap | `CompositorState` from Smithay | Double-buffered state, damage tracking, subsurface tree management |
| Client connection handling | Unix socket + epoll | `ListeningSocketSource` | Properly integrated with calloop event loop |
| Buffer lifecycle | Manual ref counting | `SurfaceAttributes` in Smithay | Handles attach/commit/release sequence automatically |
| wl_seat implementation | Manual global creation | `SeatState::new_wl_seat()` | Provides keyboard, pointer, touch in one API |
| Event loop | Custom implementation | `calloop::EventLoop` | Smithay is built around calloop's callback model |

**Key insight:** Smithay 0.7.0 handles the vast majority of protocol details. Custom implementations would miss edge cases in the Wayland specification and break protocol compliance.

## Common Pitfalls

### Pitfall 1: Missing Frame Callbacks
**What goes wrong:** Applications freeze because `wl_surface.frame` callbacks aren't sent
**Why it happens:** Wayland requires compositors to send frame callbacks to signal when to render next frame
**How to avoid:** 
- Frame callbacks are queued on surface commits
- Must be sent after frame completion (Phase 3/4)
- **Warning sign:** Client appears frozen but process is running

### Pitfall 2: Surface Role Conflicts
**What goes wrong:** "Surface already has a role" errors
**Why it happens:** Wayland surfaces can only have one role (xdg_toplevel, subsurface, cursor, etc.)
**How to avoid:**
- Check `get_role()` before assigning
- Smithay's `give_role()` returns `Result` - handle errors
- Different shells (XDG, Layer) have different roles

### Pitfall 3: Buffer Release Timing
**What goes wrong:** Client runs out of buffers or visual artifacts
**Why it happens:** Buffer must be released back to client after compositor is done reading
**How to avoid:**
- Don't release buffer until frame is fully rendered AND transmitted (Phase 4+)
- Use `add_post_commit_hook()` to track when buffers are ready
- **Critical for Phase 3:** Buffer release after RGBA extraction

### Pitfall 4: Socket Not Found
**What goes wrong:** Clients can't connect with `wayland-0` not found
**Why it happens:** Socket name must be set in `WAYLAND_DISPLAY` environment variable
**How to avoid:**
- Print socket name on startup: `/run/user/{uid}/{socket_name}`
- Users must set `WAYLAND_DISPLAY=wayland-1` (or whatever name was auto-generated)
- Socket auto-named: `wayland-0`, `wayland-1`, etc.

### Pitfall 5: Missing wl_output Causes Silent Failures
**What goes wrong:** Clients connect but don't create surfaces
**Why it happens:** Many clients require wl_output to know display parameters before creating windows
**How to avoid:**
- Always create at least one wl_output global
- Set reasonable default mode (1920x1080 @ 60Hz)
- Even headless compositors need "virtual" outputs

### Pitfall 6: Display Not in Event Loop
**What goes wrong:** Clients connect but no protocol events processed
**Why it happens:** `Display` must be added to calloop event loop to dispatch client requests
**How to avoid:**
```rust
event_loop.handle().insert_source(
    Generic::new(display, Interest::READ, Mode::Level),
    |_, display, state| {
        unsafe { display.get_mut().dispatch_clients(state).unwrap(); }
        Ok(PostAction::Continue)
    },
)?;
```

## Code Examples

### Minimal Main Loop
```rust
use smithay::reexports::calloop::EventLoop;
use smithay::reexports::wayland_server::Display;

fn main() -> anyhow::Result<()> {
    // Create event loop and display
    let mut event_loop: EventLoop<ServerState> = EventLoop::try_new()?;
    let display: Display<ServerState> = Display::new()?;
    
    // Initialize state (creates socket listener)
    let mut state = ServerState::new(&mut event_loop, display);
    
    println!("Wayland socket: {}", state.socket_name);
    
    // Run event loop
    event_loop.run(None, &mut state, |_| {})?;
    
    Ok(())
}
```

### Testing Surface Creation
```rust
// Test helper to verify surface protocol
fn test_surface_lifecycle(compositor: &mut ServerState) {
    // In real test: connect wayland-client crate
    // and verify wl_compositor.create_surface() succeeds
    
    // Verify surface exists in Smithay's internal state
    // (requires access to compositor internals or custom hooks)
}
```

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test + wayland-client crate |
| Config file | None - use Cargo.toml dev-dependencies |
| Quick run command | `cargo test --package wayland-remote-server -- --test-threads=1` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements → Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| WAYL-01 | Socket created, globals advertised | integration | `cargo test test_socket_creation` | ❌ Wave 0 |
| WAYL-01 | Client connects successfully | integration | `cargo test test_client_connection` | ❌ Wave 0 |
| WAYL-02 | Surface created on commit | unit | `cargo test test_surface_commit` | ❌ Wave 0 |
| WAYL-02 | Buffer attachment detected | unit | `cargo test test_buffer_attach` | ❌ Wave 0 |
| WAYL-03 | Surface destruction hooks called | unit | `cargo test test_surface_destruction` | ❌ Wave 0 |

### Wave 0 Gaps
- [ ] `crates/server/tests/test_core_protocol.rs` - Integration tests for WAYL-01/02/03
- [ ] `crates/server/tests/common/mod.rs` - Test utilities for Wayland client connection
- [ ] `crates/server/tests/fixtures/` - Simple Wayland client binaries for testing

### Testing Approach
Since Phase 2 is headless (no rendering), testing requires:
1. **wayland-client crate** (0.31.x) in dev-dependencies
2. **Test clients** - Simple programs that:
   - Connect to compositor
   - Create wl_surface
   - Attach buffer (can be shm buffer with dummy data)
   - Commit
3. **Verification** - Check server state through:
   - Custom hooks registered via `add_post_commit_hook()`
   - Surface tree iteration with `with_surface_tree_upward()`
   - Client disconnection monitoring via `ClientData::disconnected()`

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Smithay 0.6 | Smithay 0.7.0 | 2025-06-24 | Breaking changes in renderer APIs, xdg_shell now requires configure/ack |
| Manual Dispatch traits | `delegate_*!` macros | 0.3.0 | Much less boilerplate for protocol handlers |
| wlroots-rs | Smithay | 2020+ | Smithay is now mature, Rust-native, safer |

**Deprecated/outdated:**
- `wayland-server` 0.30: Upgrade to 0.31.9 for Smithay 0.7.0 compatibility
- `calloop` 0.13: Smithay 0.7.0 requires 0.14.0

## Open Questions

1. **Socket Permissions**
   - What we know: Socket created in `/run/user/{uid}/`
   - What's unclear: SSH remote forwarding of Wayland socket
   - Recommendation: Document socket path for users; Phase 4 handles TCP

2. **Surface Buffer Format**
   - What we know: SHM supports ARGB8888, XRGB8888
   - What's unclear: Whether clients will use dmabuf (GPU buffers)
   - Recommendation: Start with SHM only; Phase 6 adds dmabuf support

3. **Multiple Outputs**
   - What we know: Can create multiple wl_output globals
   - What's unclear: Whether to create one per remote window (Phase 6) or one virtual
   - Recommendation: Single virtual output for Phase 2; revisit in Phase 6

4. **Client Authentication**
   - What we know: Smithay supports security contexts
   - What's unclear: Whether to implement client filtering
   - Recommendation: Open access for Phase 2; add filtering in Phase 4 with TCP auth

## Sources

### Primary (HIGH confidence)
- Smithay 0.7.0 docs: https://docs.rs/smithay/0.7.0/ - Core compositor APIs
- wayland-server 0.31.9 docs: https://docs.rs/wayland-server/0.31.9/ - Protocol handling
- Smallvil example: https://github.com/Smithay/smithay/tree/master/smallvil - Minimal compositor pattern
- Anvil reference: https://github.com/Smithay/smithay/tree/master/anvil - Production patterns

### Secondary (MEDIUM confidence)
- Smithay CHANGELOG: https://github.com/Smithay/smithay/blob/master/CHANGELOG.md - Breaking changes in 0.7.0
- GETTING_STARTED.md: https://github.com/Smithay/smithay/blob/master/GETTING_STARTED.md - Architecture overview

### Tertiary (LOW confidence)
- Wayland protocol spec: https://wayland.freedesktop.org/docs/html/ - Protocol semantics
- wayland-book: https://wayland-book.com/ - Client-side perspective

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - Smithay 0.7.0 is released, docs comprehensive
- Architecture: HIGH - Smallvil/Anvil provide working examples
- Pitfalls: MEDIUM - Some pitfalls from training data (6 documented in STATE.md), others from docs

**Research date:** 2026-03-10
**Valid until:** 2026-06-10 (Smithay releases are infrequent; check CHANGELOG if extending)

---

**Next Steps for Planner:**
1. Create PLAN.md based on Smallvil pattern
2. Priority: Socket creation → CompositorState init → Client connection test
3. Leave rendering (Phase 3) and XDG shell (Phase 7) for future phases
4. Consider testing approach: wayland-client dev-dependency with simple test client

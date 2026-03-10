//! Server state for Wayland compositor
//! 
//! Implements the Smallvil pattern: minimal compositor with CompositorState,
//! integrated with calloop event loop for event dispatch.

use smithay::reexports::{
    calloop::{EventLoop, generic::Generic, Interest, Mode, PostAction},
    wayland_server::{
        Display, DisplayHandle, protocol::wl_surface::WlSurface, Client,
        backend::{ClientData, DisconnectReason, ClientId},
    },
};
use smithay::wayland::{
    compositor::{CompositorClientState, CompositorState, CompositorHandler, with_states},
    socket::ListeningSocketSource
};
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::output::Output;
use smithay::delegate_compositor;
use smithay::delegate_seat;
use std::sync::atomic::{AtomicU32, Ordering};
use wayland_remote_server::handlers::{seat, output};
use std::sync::Arc;
use tracing::info;

/// Server state holding all compositor-related state
/// 
/// This struct implements all the Smithay handler traits and holds the
/// core Wayland protocol state (compositor, seat, output, etc.).
pub struct ServerState {
    /// Handle to the Wayland display for creating globals
    pub display_handle: DisplayHandle,
    /// Compositor state for managing surfaces
    pub compositor_state: CompositorState,
    /// Seat state for wl_seat global
    pub seat_state: SeatState<Self>,
    /// Seat with keyboard and pointer capabilities
    pub seat: Seat<Self>,
    /// Virtual output for wl_output global
    pub output: Output,
    /// Name of the Wayland socket (e.g., "wayland-0")
    pub socket_name: std::ffi::OsString,
    /// Serial counter for input events
    serial_counter: AtomicU32,
}

impl ServerState {
    /// Initialize the server state
    /// 
    /// Creates the compositor state, sets up the listening socket,
    /// and integrates both into the calloop event loop.
    /// 
    /// # Arguments
    /// * `event_loop` - The calloop event loop to register sources with
    /// * `display` - The Wayland display to initialize
    /// 
    /// # Returns
    /// Self containing the initialized compositor state and socket name
    pub fn new(event_loop: &mut EventLoop<Self>, display: Display<Self>) -> Self {
        let dh = display.handle();
        
        // Initialize compositor state - this advertises wl_compositor global
        let compositor_state = CompositorState::new::<Self>(&dh);
        
        info!("Compositor state initialized, wl_compositor global advertised");
        
        // Initialize seat state - this advertises wl_seat global with keyboard and pointer
        let (seat_state, seat) = seat::create_seat(&dh, "wayland-remote-seat");
        info!("Seat state initialized, wl_seat global advertised with keyboard and pointer");
        
        // Initialize output - this advertises wl_output global
        let output = output::create_virtual_output(&dh);
        info!("Output initialized, wl_output global advertised with 1920x1080 @ 60Hz mode");
        // Setup listening socket for client connections
        // ListeningSocketSource::new_auto() creates socket in XDG_RUNTIME_DIR
        // with auto-generated name like "wayland-0", "wayland-1", etc.
        let listening_socket = ListeningSocketSource::new_auto()
            .expect("Failed to create Wayland listening socket");
        
        let socket_name = listening_socket.socket_name().to_os_string();
        
        info!("Wayland listening socket created: {}", 
              socket_name.to_string_lossy());
        
        // Insert socket source into event loop
        // This handles accepting new client connections
        event_loop.handle()
            .insert_source(listening_socket, |client_stream, _, state| {
                // When a client connects, insert it into the display
                // with a default ClientState
                state.display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                    .expect("Failed to insert client");
            })
            .expect("Failed to insert Wayland socket source into event loop");
        
        // Insert Display source into event loop
        // This handles dispatching Wayland protocol events
        event_loop.handle()
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    // Dispatch all pending Wayland events
                    // This is unsafe because we're mutating the display
                    // while holding a reference, but Smithay guarantees safety
                    unsafe {
                        display.get_mut().dispatch_clients(state).unwrap();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .expect("Failed to insert Display source into event loop");
        
        Self {
            display_handle: dh,
            compositor_state,
            seat_state,
            seat,
            output,
            socket_name,
            serial_counter: AtomicU32::new(0),
        }
    }
    
    /// Get the full socket path for clients
    /// 
    /// Returns the path in format: /run/user/{uid}/{socket_name}
    pub fn socket_path(&self) -> std::ffi::OsString {
        let runtime_dir = std::env::var("XDG_RUNTIME_DIR")
            .unwrap_or_else(|_| format!("/run/user/{}", std::env::var("UID").unwrap_or_else(|_| "0".to_string())));
        
        let mut path = std::ffi::OsString::from(runtime_dir);
        path.push("/");
        path.push(&self.socket_name);
        path
    }
}

/// Per-client state
/// 
/// Holds state specific to each connected Wayland client.
/// Currently minimal as we're just accepting connections.
#[derive(Default)]
pub struct ClientState {
    /// Per-client compositor state
    pub compositor_state: CompositorClientState,

}

/// Implement ClientData trait for ClientState
/// 
/// This trait provides hooks for client lifecycle events.
impl ClientData for ClientState {
    /// Called when a client completes its initial protocol setup
    fn initialized(&self, _client_id: ClientId) {
        // Client is ready to use
    }
    
    /// Called when a client disconnects
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {
        // Client disconnected - cleanup happens automatically
        // via CompositorState's internal tracking
    }
}

/// Implement CompositorHandler for ServerState
/// 
/// This trait handles surface lifecycle events.
impl CompositorHandler for ServerState {
    /// Get mutable reference to compositor state
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }
    
    /// Get per-client compositor state
    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client
            .get_data::<ClientState>()
            .unwrap()
            .compositor_state
    }
    
    /// Called when a surface commits new state
    /// 
    /// This is where we'd handle buffer attachment, damage tracking, etc.
    /// For Phase 2, we just log the commit. Phase 3 will handle rendering.
    fn commit(&mut self, surface: &WlSurface) {
        // Access surface state to check what changed
        with_states(surface, |states| {
            // In Phase 3, we'll extract buffer data here
            // For now, just acknowledge the commit
            drop(states.cached_state.get::<smithay::wayland::compositor::SurfaceAttributes>());
        });
    }
}

delegate_compositor!(ServerState);




/// Implement SeatHandler for ServerState
///
/// This trait provides access to seat state for protocol delegation.
impl SeatHandler for ServerState {
    /// Type for keyboard focus - using WlSurface which implements WaylandFocus
    type KeyboardFocus = smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
    
    /// Type for pointer focus
    type PointerFocus = smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
    
    /// Type for touch focus
    type TouchFocus = smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
    
    /// Get mutable reference to seat state
    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

}

delegate_seat!(ServerState);

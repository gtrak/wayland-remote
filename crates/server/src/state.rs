//! 
//! Implements the Smallvil pattern: minimal compositor with CompositorState,
//! integrated with calloop event loop for event dispatch.

use smithay::reexports::{
    calloop::{EventLoop, generic::Generic, Interest, Mode, PostAction},
    wayland_server::{
        Display, DisplayHandle, protocol::wl_surface::WlSurface, Client, Resource,
        backend::{ClientData, DisconnectReason, ClientId},
    },
};
use smithay::wayland::{
    compositor::{CompositorClientState, CompositorState, CompositorHandler, SurfaceAttributes, with_states, BufferAssignment},
    socket::ListeningSocketSource,
    shm::{ShmState, ShmHandler},
    buffer::BufferHandler,
    shell::xdg::{XdgShellState, XdgShellHandler, ToplevelSurface, PopupSurface, PositionerState},
};
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::output::Output;
use smithay::wayland::output::{OutputManagerState, OutputHandler};
use smithay::delegate_compositor;
use smithay::delegate_seat;
use smithay::delegate_output;
use smithay::delegate_shm;
use smithay::delegate_xdg_shell;
use smithay::utils::Serial;
use wayland_server::backend::ObjectId;
use std::collections::HashMap;
use std::sync::atomic::AtomicU32;
use std::sync::Arc;
use std::time::Instant;
use tracing::info;
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::renderer::ImportMemWl;
use smithay::backend::renderer::Texture;
use smithay::reexports::pixman::Image;
// Import rendering functions
use wayland_remote_server::rendering::offscreen;
use wayland_remote_server::rendering::pixel_export::{self, RgbaData};

// Import streaming module
use wayland_remote_server::streaming::{StreamingServer, StreamingState, FrameData};
use wayland_remote_server::streaming::surface::SurfaceTracker;
use tokio::sync::RwLock;
use wayland_remote_server::handlers::{seat, output};

/// Information about a tracked surface
#[derive(Debug, Clone)]
pub struct SurfaceInfo {
    #[allow(dead_code)]
    /// When the surface was created
    pub creation_time: Instant,
    /// Number of buffer attachments
    pub buffer_count: u32,
    /// Last commit time
    pub last_commit: Option<Instant>,
}

impl SurfaceInfo {
    pub fn new() -> Self {
        Self {
            creation_time: Instant::now(),
            buffer_count: 0,
            last_commit: None,
        }
    }
}

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
    /// Output manager state for wl_output global
    pub output_manager_state: OutputManagerState,
    /// Virtual output for wl_output global
    pub output: Output,
    /// Shared memory state for wl_shm global
    pub shm_state: ShmState,
    /// XDG Shell state for window management (S07)
    pub xdg_shell_state: XdgShellState,
    /// Name of the Wayland socket (e.g., "wayland-0")
    pub socket_name: std::ffi::OsString,
    /// Serial counter for input events
    serial_counter: AtomicU32,
    /// Track active surfaces for lifecycle management
    pub surfaces: HashMap<ObjectId, SurfaceInfo>,
    /// PixmanRenderer for headless software rendering
    pub renderer: PixmanRenderer,
    /// Per-surface offscreen buffer tracking for frame capture (REND-02)
    pub offscreen_buffers: HashMap<ObjectId, Image<'static, 'static>>,
    /// Per-surface captured RGBA frames for streaming (REND-03)
    pub captured_frames: HashMap<ObjectId, RgbaData>,
    /// Surface tracker for unique window ID management (STREAM-04)
    pub surface_tracker: Arc<SurfaceTracker>,
    /// Streaming server configuration (STREAM-01, STREAM-02)
    pub streaming_server: StreamingServer,
    /// Streaming state for TCP frame delivery
    pub streaming_state: Arc<RwLock<StreamingState>>,
    /// Maps toplevel surfaces to their window IDs for window management (S07)
    pub toplevel_windows: HashMap<ObjectId, u32>,
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
        
        // Initialize PixmanRenderer for headless software rendering (REND-01)
        let renderer = PixmanRenderer::new().expect("Failed to create PixmanRenderer");
        info!("PixmanRenderer initialized for headless software rendering");
        
        // Initialize compositor state - this advertises wl_compositor global
        let compositor_state = CompositorState::new::<Self>(&dh);
        
        info!("Compositor state initialized, wl_compositor global advertised");
        
        // Initialize ShmState for shared memory buffers (M-2)
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        info!("ShmState initialized, wl_shm global advertised");
        
        // Initialize XDG Shell state for window management (S07)
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        info!("XDG Shell state initialized, xdg_wm_base global advertised");
        
        // Initialize seat state - this advertises wl_seat global with keyboard and pointer
        let (seat_state, seat) = seat::create_seat(&dh, "wayland-remote-seat");
        info!("Seat state initialized, wl_seat global advertised with keyboard and pointer");
        
        // Initialize output manager state - this advertises wl_output global
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        
        // Create virtual output
        let output = output::create_virtual_output::<ServerState>(&dh);
        
        // Configure the output with mode
        output.change_current_state(
            Some(smithay::output::Mode { size: smithay::utils::Size::new(1920, 1080), refresh: 60000 }),
            Some(smithay::utils::Transform::Normal),
            Some(smithay::output::Scale::Integer(1)),
            Some(smithay::utils::Point::new(0, 0)),
        );
        output.set_preferred(smithay::output::Mode { size: smithay::utils::Size::new(1920, 1080), refresh: 60000 });
        
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
            output_manager_state,
            output,
            shm_state,
            xdg_shell_state,
            renderer,
            socket_name,
            serial_counter: AtomicU32::new(0),
            surfaces: HashMap::new(),
            offscreen_buffers: HashMap::new(),
            captured_frames: HashMap::new(),
            surface_tracker: Arc::new(SurfaceTracker::new()),
            streaming_server: StreamingServer::new(6080),
            streaming_state: Arc::new(RwLock::new(StreamingState::new())),
            toplevel_windows: HashMap::new(),
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
    
    /// Get frames ready for streaming
    ///
    /// Converts captured_frames (HashMap<ObjectId, RgbaData>) to streaming format
    /// with stable window_id mapping for TCP transmission.
    ///
    /// # Returns
    /// HashMap<window_id, FrameData> ready for streaming
    pub fn get_frames_for_streaming(&mut self) -> std::collections::HashMap<u32, FrameData> {
        let timestamp_us = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_micros() as u64;

        self.captured_frames
            .iter()
            .map(|(surface_id, rgba)| {
                // Allocate window ID using SurfaceTracker
                let window_id = self.surface_tracker.allocate_window_id(surface_id.clone());

                (window_id, FrameData::new(
                    rgba.width,
                    rgba.height,
                    timestamp_us,
                    rgba.data.clone(),
                ))
            })
            .collect()
    }
    
    /// Update streaming state with newly captured frames
    ///
    /// Called after frame capture to make frames available for streaming.
    pub async fn update_streaming_state(&mut self) {
        let frames = self.get_frames_for_streaming();
        let state = self.streaming_state.write().await;
        
        for (window_id, frame) in frames {
            state.surfaces.write().await.insert(window_id, frame);
        }
    }
    
    /// Remove a surface from streaming state
    ///
    /// Called when a surface is destroyed.
    pub async fn remove_streaming_surface(&mut self, surface_id: ObjectId) {
        // Remove from captured_frames
        self.captured_frames.remove(&surface_id);

        // Remove from SurfaceTracker
        let window_id = self.surface_tracker.remove_surface(surface_id.clone());
        
        // Remove from streaming state
        let mut state = self.streaming_state.write().await;
        if let Some(wid) = window_id {
            state.remove_surface(wid).await;
        }
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
    /// This is the core of surface lifecycle tracking:
    /// - Detect buffer attachments via SurfaceAttributes (M-3)
    /// - Track surface commits
    /// - Log surface activity for debugging
    fn commit(&mut self, surface: &WlSurface) {
        // Get surface ID for tracking
        let surface_id = surface.id();
        
        // Use with_states to access SurfaceAttributes and detect buffer attachments (M-3)
        let buffer_attached = with_states(surface, |states| {
            let mut attrs = states.cached_state.get::<SurfaceAttributes>();
            attrs.current().buffer.is_some()
        });
        
        // Track surface in ServerState
        self.surfaces
            .entry(surface_id.clone())
            .and_modify(|info| {
                // Update existing surface info
                info.last_commit = Some(Instant::now());
                if buffer_attached {
                    info.buffer_count += 1;
                }
            })
            .or_insert_with(|| SurfaceInfo {
                creation_time: Instant::now(),
                buffer_count: if buffer_attached { 1 } else { 0 },
                last_commit: Some(Instant::now()),
            });
        
        // Render surface to offscreen buffer when buffer is attached (REND-02)
        if buffer_attached {
            // Get surface dimensions from buffer by importing it
            let result = with_states(surface, |states| {
                let mut attrs = states.cached_state.get::<SurfaceAttributes>();
                if let Some(BufferAssignment::NewBuffer(buf)) = &attrs.current().buffer {
                    // Import the buffer to get its size
                    match self.renderer.import_shm_buffer(buf, Some(states), &[]) {
                        Ok(texture) => Some(texture.size()),
                        Err(_) => None,
                    }
                } else {
                    None
                }
            });
            
            if let Some(size) = result {
                let width = size.w;
                let height = size.h;
                
                // Check if buffer exists and dimensions match (M-1: buffer resize handling)
                let needs_new_buffer = self.offscreen_buffers.get(&surface_id)
                    .map(|buf| buf.width() as i32 != width || buf.height() as i32 != height)
                    .unwrap_or(true);
                
                if needs_new_buffer {
                    // Remove old buffer if it exists
                    self.offscreen_buffers.remove(&surface_id);
                }
                
                // Get or create offscreen buffer for this surface
                let buffer = match self.offscreen_buffers.get_mut(&surface_id) {
                    Some(buf) => buf,
                    None => {
                        // Create new buffer with surface dimensions (M-2: proper error handling)
                        match offscreen::create_offscreen_buffer(&mut self.renderer, width, height) {
                            Ok(buffer) => {
                                let _existing = self.offscreen_buffers.insert(surface_id.clone(), buffer);
                                // Safety: We just inserted, so this unwrap is safe
                                self.offscreen_buffers.get_mut(&surface_id).unwrap()
                            }
                            Err(e) => {
                                tracing::error!("Failed to create offscreen buffer for surface {:?}: {}", surface_id, e);
                                return; // M-2: graceful error handling - return early
                            }
                        }
                    }
                };
                
                // Render surface to buffer
                if !offscreen::try_render_surface_to_buffer(&mut self.renderer, surface, buffer) {
                    tracing::warn!("Failed to render surface {:?} to offscreen buffer", surface_id);
                } else {
                    info!("Surface {:?}: Rendered to offscreen buffer ({})", surface_id, width * height);
                    
                    // Extract RGBA pixel data from the rendered buffer (REND-03)
                    // Buffer is held until extraction completes per Pattern 4
                    if let Some(rgba_data) = pixel_export::extract_rgba_pixels(&mut self.renderer, surface, buffer) {
                        // M-3: Remove old frame before inserting new one to prevent unbounded memory growth
                        self.captured_frames.remove(&surface_id);
                        self.captured_frames.insert(surface_id.clone(), rgba_data.clone());
                        info!("Surface {:?}: Extracted RGBA pixels ({} bytes)", surface_id, rgba_data.byte_size());
                    }
                }
            }
        }
        
        // Log with buffer attachment status
        if buffer_attached {
            let buffer_count = self.surfaces.get(&surface_id).map(|s| s.buffer_count).unwrap_or(0);
            info!("Surface {:?}: Commit received with buffer attachment (total buffers: {})", surface_id, buffer_count);
        } else {
            info!("Surface {:?}: Commit received (no buffer)", surface_id);
        }
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

/// Implement OutputHandler for ServerState
///
/// This trait handles wl_output binding events.
impl OutputHandler for ServerState {
    // Default implementation does nothing, which is fine for our use case
}

delegate_output!(ServerState);

/// Implement ShmHandler for ServerState
///
/// This trait handles wl_shm binding events.
impl ShmHandler for ServerState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

/// Implement BufferHandler for ServerState (required for ShmState)
impl BufferHandler for ServerState {
    fn buffer_destroyed(&mut self, _buffer: &wayland_server::protocol::wl_buffer::WlBuffer) {
        // Buffer destroyed - no custom cleanup needed
        // The surfaces HashMap is cleaned up when clients disconnect
    }
}

delegate_shm!(ServerState);

/// Implement XdgShellHandler for ServerState
///
/// This trait handles XDG Shell protocol events for window management (S07).
/// It tracks toplevel surfaces and associates them with window IDs.
impl XdgShellHandler for ServerState {
    /// Get mutable reference to XDG Shell state
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }
    
    /// Called when a new toplevel surface is created
    ///
    /// This is triggered when a client creates an xdg_toplevel surface.
    /// We associate the toplevel with a window ID for remote streaming.
    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let surface_id = surface.wl_surface().id();
        
        // Allocate a window ID for this toplevel surface
        let window_id = self.surface_tracker.allocate_window_id(surface_id.clone());
        
        // Store the mapping from surface to window ID
        self.toplevel_windows.insert(surface_id.clone(), window_id);
        
        info!(
            surface_id = ?surface_id,
            window_id = window_id,
            "XDG Toplevel created - window mapped for streaming"
        );
    }
    
    /// Called when a new popup surface is created
    ///
    /// Popups are temporary surfaces like menus and tooltips.
    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {
        // Popups are tracked by Smithay but not assigned window IDs
        // They are rendered as part of their parent toplevel
        info!("XDG Popup created");
    }
    
    /// Called when a grab request is made on a popup
    fn grab(&mut self, _surface: PopupSurface, _seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat, _serial: Serial) {
        // Grab handling for dismiss-on-click-outside behavior
        info!("XDG Popup grab requested");
    }
    
    /// Called when a reposition request is made on a popup
    fn reposition_request(&mut self, _surface: PopupSurface, _positioner: PositionerState, _token: u32) {
        // Reposition handling for popup placement
        info!("XDG Popup reposition requested");
    }
}

delegate_xdg_shell!(ServerState);

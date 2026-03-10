//! TCP Frame Streaming Module
//!
//! Provides TCP server infrastructure for streaming captured RGBA frames
//! to connected Windows viewers.
//!
//! Architecture:
//! - `StreamingServer`: TCP listener that accepts viewer connections
//! - `StreamingState`: Shared state for surfaces and clients
//! - `protocol`: Binary frame encoding/decoding
//!
//! Integration with calloop:
//! Uses `calloop::futures::Executor` to run Tokio-based TCP server
//! within the calloop event loop used for Wayland protocol handling.

pub mod protocol;

use std::sync::Arc;
use std::collections::HashMap;
use tokio::sync::RwLock;
use std::net::SocketAddr;

/// Streaming server for TCP frame delivery
///
/// Manages the TCP listener and per-client connections.
/// Integrates with calloop event loop via `calloop::futures::Executor`.
#[derive(Debug, Clone)]
pub struct StreamingServer {
    /// TCP port for viewer connections
    pub port: u16,
    /// Shared streaming state
    pub state: Arc<RwLock<StreamingState>>,
}

impl StreamingServer {
    /// Create a new streaming server with the specified port
    pub fn new(port: u16) -> Self {
        Self {
            port,
            state: Arc::new(RwLock::new(StreamingState::new())),
        }
    }

    /// Get the server address for binding
    pub fn bind_address(&self) -> String {
        format!("0.0.0.0:{}", self.port)
    }
}

/// Shared state for frame streaming
///
/// Tracks surfaces (windows) and connected clients.
/// Shared via `Arc<RwLock<>>` across spawned tasks.
#[derive(Debug, Default, Clone)]
pub struct StreamingState {
    /// Surface frames ready for streaming
    /// Maps window_id -> frame data
    pub surfaces: Arc<RwLock<HashMap<u32, FrameData>>>,
    /// Connected viewer clients
    /// Maps client address -> client handle
    pub clients: Arc<RwLock<HashMap<SocketAddr, ClientHandle>>>,
}

impl StreamingState {
    /// Create a new streaming state
    pub fn new() -> Self {
        Self {
            surfaces: Arc::new(RwLock::new(HashMap::new())),
            clients: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Add or update a surface frame
    pub async fn update_frame(&mut self, window_id: u32, frame: FrameData) {
        self.surfaces.write().await.insert(window_id, frame);
    }

    /// Get all surfaces for streaming
    pub async fn get_all_surfaces(&self) -> HashMap<u32, FrameData> {
        self.surfaces.read().await.clone()
    }

    /// Remove a surface (e.g., when window is destroyed)
    pub async fn remove_surface(&mut self, window_id: u32) {
        self.surfaces.write().await.remove(&window_id);
    }

    /// Register a connected client
    pub async fn register_client(&mut self, addr: SocketAddr, handle: ClientHandle) {
        self.clients.write().await.insert(addr, handle);
    }

    /// Unregister a disconnected client
    pub async fn unregister_client(&mut self, addr: SocketAddr) {
        self.clients.write().await.remove(&addr);
    }

    /// Get number of connected clients
    pub async fn client_count(&self) -> usize {
        self.clients.read().await.len()
    }
}

/// Frame data ready for streaming
///
/// Contains the header metadata and raw RGBA pixel data.
#[derive(Debug, Clone)]
pub struct FrameData {
    /// Frame dimensions
    pub width: u32,
    /// Frame height
    pub height: u32,
    /// Unix timestamp in microseconds
    pub timestamp_us: u64,
    /// Raw RGBA pixel bytes (width × height × 4)
    pub rgba: Vec<u8>,
}

impl FrameData {
    /// Create new frame data
    pub fn new(width: u32, height: u32, timestamp_us: u64, rgba: Vec<u8>) -> Self {
        Self {
            width,
            height,
            timestamp_us,
            rgba,
        }
    }

    /// Calculate total byte size of the frame
    pub fn byte_size(&self) -> usize {
        self.width as usize * self.height as usize * 4
    }
}

/// Handle for a connected client
///
/// Used to track client state and communication channels.
#[derive(Debug, Clone)]
pub struct ClientHandle {
    /// Client address
    pub addr: SocketAddr,
    /// Connected timestamp
    pub connected_at: std::time::Instant,
}

impl ClientHandle {
    /// Create a new client handle
    pub fn new(addr: SocketAddr) -> Self {
        Self {
            addr,
            connected_at: std::time::Instant::now(),
        }
    }
}

/// Start the streaming server
///
/// This is the entry point for the TCP server. It binds to the configured
/// port and accepts viewer connections in a loop.
///
/// Note: This function is async and should be spawned via
/// `calloop::futures::Executor` to integrate with the calloop event loop.
///
/// # Arguments
/// * `server` - The streaming server configuration
///
/// # Returns
/// Result indicating success or failure
pub async fn start_streaming_server(server: &StreamingServer) -> anyhow::Result<()> {
    use tokio::net::TcpListener;
    use tracing::{info, error, warn};

    let addr = server.bind_address();
    info!("Starting TCP streaming server on {}", addr);

    let listener = TcpListener::bind(&addr).await
        .map_err(|e| anyhow::anyhow!("Failed to bind TCP listener: {}", e))?;

    info!("TCP streaming server listening on {}", addr);

    // Accept loop - spawn a task for each client
    loop {
        match listener.accept().await {
            Ok((socket, addr)) => {
                info!("New viewer connection from {}", addr);
                
                let state = server.state.clone();
                
                // Spawn task for handling this client
                tokio::spawn(async move {
                    if let Err(e) = handle_client(socket, addr, state).await {
                        warn!("Client {} error: {}", addr, e);
                    }
                });
            }
            Err(e) => {
                error!("Failed to accept connection: {}", e);
            }
        }
    }
}

/// Handle a single client connection
///
/// This is a placeholder for the per-client handler that will be
/// implemented in Plan 02 (actual frame sending).
///
/// # Arguments
/// * `socket` - The TCP socket for the client
/// * `addr` - Client address
/// * `state` - Shared streaming state
async fn handle_client(
    _socket: tokio::net::TcpStream,
    addr: SocketAddr,
    state: Arc<RwLock<StreamingState>>,
) -> anyhow::Result<()> {
    use tracing::info;

    // Register client
    let handle = ClientHandle::new(addr);
    state.write().await.register_client(addr, handle.clone()).await;

    info!("Client {} registered ({} clients total)", 
          addr, state.read().await.client_count().await);

    // TODO: Implement frame sending logic in Plan 02
    // For now, just wait for socket to close
    
    // Cleanup on disconnect
    state.write().await.unregister_client(addr).await;
    info!("Client {} disconnected", addr);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_streaming_server_new() {
        let server = StreamingServer::new(6080);
        assert_eq!(server.port, 6080);
        assert_eq!(server.bind_address(), "0.0.0.0:6080");
    }

    #[test]
    fn test_frame_data_byte_size() {
        let frame = FrameData::new(1920, 1080, 0, vec![0u8; 1920 * 1080 * 4]);
        assert_eq!(frame.byte_size(), 1920 * 1080 * 4);
    }

    #[tokio::test]
    async fn test_streaming_state() {
        let mut state = StreamingState::new();
        
        // Add a frame
        let frame = FrameData::new(100, 100, 1000000, vec![0xFFu8; 100 * 100 * 4]);
        state.update_frame(42, frame).await;
        
        // Retrieve it
        let surfaces = state.get_all_surfaces().await;
        assert!(surfaces.contains_key(&42));
        assert_eq!(surfaces[&42].width, 100);
        assert_eq!(surfaces[&42].height, 100);
        
        // Remove it
        state.remove_surface(42).await;
        assert!(!state.get_all_surfaces().await.contains_key(&42));
    }

    #[tokio::test]
    async fn test_client_tracking() {
        let mut state = StreamingState::new();
        
        let addr: SocketAddr = "127.0.0.1:12345".parse().unwrap();
        let handle = ClientHandle::new(addr);
        
        state.register_client(addr, handle).await;
        assert_eq!(state.client_count().await, 1);
        
        state.unregister_client(addr).await;
        assert_eq!(state.client_count().await, 0);
    }
}

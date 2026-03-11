//! Viewer application with winit ApplicationHandler
//!
//! This module implements the main application loop using winit's
//! ApplicationHandler trait to manage window lifecycle and frame rendering.
//!
//! # Architecture
//!
//! ```
//! ┌─────────────────┐     mpsc      ┌──────────────────┐
//! │   Network Thread │ ───────────▶ │   Main Thread    │
//! │  (Tokio Runtime) │   Frame      │ (Winit Event Loop│
//! │                  │              │                  │
//! │  TcpClient::     │              │  ViewerApp::     │
//! │   read_frame()   │              │   window_event() │
//! └─────────────────┘              └────────┬─────────┘
//!        ▲                                  │
//!        │                                  │
//!        └──────────────────────────────────┘
//!                         StretchDIBits()
//! ```

#![cfg(windows)]

use std::sync::mpsc;
use std::thread;

use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::event::StartCause;
use winit::window::WindowId;

use tracing::{debug, error, info, warn};

use crate::display::DisplayWindow;
use crate::network::{Frame, TcpClient};

/// Channel buffer size for frame streaming
/// Allows buffering multiple frames to handle network bursts
const FRAME_BUFFER_SIZE: usize = 10;

/// Main viewer application
///
/// Manages the window, network connection, and frame rendering pipeline.
pub struct ViewerApp {
    /// Display window (created after event loop is available)
    display_window: Option<DisplayWindow>,
    /// Server address to connect to
    server_address: String,
    /// Frame receiver channel from network thread
    frame_rx: Option<mpsc::Receiver<Frame>>,
}

impl ViewerApp {
    /// Create a new viewer application
    ///
    /// # Arguments
    /// * `server_address` - Address of the remote compositor server
    ///
    /// # Returns
    /// A new ViewerApp instance
    pub fn new(server_address: impl Into<String>) -> Self {
        Self {
            display_window: None,
            server_address: server_address.into(),
            frame_rx: None,
        }
    }

    /// Set the frame receiver channel
    ///
    /// # Arguments
    /// * `frame_rx` - Receiver end of the frame channel
    pub fn set_frame_receiver(&mut self, frame_rx: mpsc::Receiver<Frame>) {
        self.frame_rx = Some(frame_rx);
    }

    /// Process pending frames from the receiver
    ///
    /// Reads all available frames and submits them to the display window.
    fn process_frames(&mut self) {
        if let Some(ref mut rx) = self.frame_rx {
            // Process all available frames
            while let Ok(frame) = rx.try_recv() {
                if let Some(ref mut window) = self.display_window {
                    debug!(
                        window_id = frame.header.window_id,
                        width = frame.header.width,
                        height = frame.header.height,
                        "Processing frame"
                    );
                    window.submit_frame(&frame);
                }
            }
        }
    }
}

impl ApplicationHandler for ViewerApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        // Create window on resume (winit 0.30 best practice)
        if self.display_window.is_none() {
            // Default window size (will be updated by first frame)
            let default_width = 800;
            let default_height = 600;

            let window = DisplayWindow::new(
                event_loop,
                "Wayland Remote Viewer",
                default_width,
                default_height,
            );

            info!(
                width = default_width,
                height = default_height,
                "Created display window"
            );

            self.display_window = Some(window);
        }
    }

    fn finished(&mut self, _event_loop: &ActiveEventLoop) {
        info!("Application finished");
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: StartCause) {
        // Process any pending frames on new events
        self.process_frames();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            winit::event::WindowEvent::CloseRequested => {
                info!("Window closed, shutting down");
                // Exit the event loop
                event_loop.exit();
            }
            winit::event::WindowEvent::RedrawRequested => {
                // Render the current frame
                if let Some(ref window) = self.display_window {
                    window.on_paint();
                }
            }
            winit::event::WindowEvent::Resized(size) => {
                debug!(width = size.width, height = size.height, "Window resized");
                // Window will automatically redraw on resize
            }
            _ => {
                // Ignore other events
            }
        }
    }
}

/// Spawn the network client in a dedicated thread
///
/// This function creates a Tokio runtime in a separate thread, connects to
/// the server, and receives frames via an mpsc channel.
///
/// # Arguments
/// * `server_address` - Address of the remote compositor server
/// * `frame_tx` - Sender end of the frame channel
///
/// # Returns
/// Join handle for the network thread
fn spawn_network_thread(
    server_address: String,
    frame_tx: mpsc::Sender<Frame>,
    shutdown_rx: mpsc::Receiver<()>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        // Create a dedicated Tokio runtime for network operations
        let rt = tokio::runtime::Runtime::new()
            .expect("Failed to create Tokio runtime");

        rt.block_on(async move {
            info!(address = %server_address, "Starting network client");

            let client = TcpClient::new(&server_address);

            loop {
                // Check for shutdown signal
                if shutdown_rx.try_recv().is_ok() {
                    info!("Shutdown signal received, stopping network thread");
                    break;
                }

                match client.connect().await {
                    Ok(stream) => {
                        info!(address = %server_address, "Connected to server");

                        // Start receiving frames
                        let mut rx = TcpClient::start_receiving(stream, FRAME_BUFFER_SIZE).await;

                        // Forward frames to the main thread
                        while let Some(frame) = rx.recv().await {
                            // Check for shutdown signal
                            if shutdown_rx.try_recv().is_ok() {
                                info!("Shutdown signal received, stopping network thread");
                                break;
                            }

                            if frame_tx.send(frame).is_err() {
                                warn!("Frame receiver dropped, stopping network thread");
                                break;
                            }
                        }

                        // Channel closed or error occurred
                        warn!("Connection lost, attempting to reconnect in 1 second...");
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                    Err(e) => {
                        error!(address = %server_address, error = %e, "Failed to connect to server");
                        warn!("Retrying in 1 second...");
                        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    }
                }
            }
        });

        info!("Network thread exiting");
    })
}

/// Run the viewer application
///
/// This function creates the event loop, initializes the application,
/// spawns the network thread, and starts the main loop.
///
/// # Arguments
/// * `server_address` - Address of the remote compositor server
///
/// # Returns
/// Result indicating success or failure
pub fn run(server_address: impl Into<String>) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing (should already be done in main, but ensure it's set)
    tracing_subscriber::fmt::try_init().ok();

    info!("Starting Wayland Remote Viewer");

    // Create channel for frame streaming from network thread to UI thread
    let (frame_tx, frame_rx) = mpsc::channel::<Frame>(FRAME_BUFFER_SIZE);

    // Create shutdown channel for graceful network thread termination
    let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();

    // Spawn network thread
    let server_address_str = server_address.into();
    let network_thread = spawn_network_thread(server_address_str.clone(), frame_tx, shutdown_rx);

    info!(address = %server_address_str, "Network thread spawned");

    // Create event loop
    let event_loop = EventLoop::new().expect("Failed to create event loop");

    // Create application
    let mut app = ViewerApp::new(&server_address_str);
    app.set_frame_receiver(frame_rx);

    info!("Starting event loop");

    // Run event loop (window will be created in resumed())
    event_loop.run_app(&mut app)?;

    // Signal network thread to shut down gracefully
    info!("Signaling network thread to shut down");
    let _ = shutdown_tx.send(());

    // Wait for network thread to finish
    info!("Waiting for network thread to exit");
    if let Err(e) = network_thread.join() {
        warn!("Network thread panicked: {:?}", e);
    }

    info!("Application exited");
    Ok(())

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        let app = ViewerApp::new("127.0.0.1:8080");
        assert_eq!(app.server_address, "127.0.0.1:8080");
        assert!(app.display_window.is_none());
    }
}

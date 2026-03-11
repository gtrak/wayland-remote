//! Viewer application with winit ApplicationHandler
//!
//! This module implements the main application loop using winit's
//! ApplicationHandler trait to manage window lifecycle and frame rendering.

#![cfg(windows)]

use std::sync::mpsc;

use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::Window;

use tracing::{debug, error, info, warn};

use crate::display::DisplayWindow;
use crate::network::{Frame, TcpClient};

/// Main viewer application
///
/// Manages the window, network connection, and frame rendering pipeline.
pub struct ViewerApp {
    /// Display window (created after event loop is available)
    display_window: Option<DisplayWindow>,
    /// Server address to connect to
    server_address: String,
    /// Frame receiver channel
    frame_rx: Option<mpsc::Receiver<Frame>>,
    /// Whether the app should continue running
    running: bool,
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
            running: true,
        }
    }

    /// Initialize the display window
    ///
    /// Called when the event loop is ready.
    pub fn init_window(&mut self, event_loop: &ActiveEventLoop) {
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
                        width = frame.header.width,
                        height = frame.header.height,
                        "Processing frame"
                    );
                    window.submit_frame(&frame);
                }
            }
        }
    }

    /// Handle window close event
    fn on_close(&mut self) {
        info!("Window closed, shutting down");
        self.running = false;
    }
}

impl ApplicationHandler for ViewerApp {
    fn finished(&mut self, _event_loop: &ActiveEventLoop) {
        info!("Application finished");
    }

    fn new_events(&mut self, _event_loop: &ActiveEventLoop, _cause: winit::event::StartCause) {
        // Process any pending frames
        self.process_frames();
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: winit::window::WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.on_close();
            }
            WindowEvent::RedrawRequested => {
                // Render the current frame
                if let Some(ref window) = self.display_window {
                    window.on_paint();
                }
            }
            WindowEvent::Resized(size) => {
                debug!(width = size.width, height = size.height, "Window resized");
                // Window will automatically redraw on resize
            }
            _ => {
                // Ignore other events
            }
        }
    }
}

/// Run the viewer application
///
/// This function creates the event loop, initializes the application,
/// and starts the main loop.
///
/// # Arguments
/// * `server_address` - Address of the remote compositor server
///
/// # Returns
/// Result indicating success or failure
pub fn run(server_address: impl Into<String>) -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::fmt::init();

    info!("Starting Wayland Remote Viewer");

    // Create event loop
    let event_loop = EventLoop::new().expect("Failed to create event loop");

    // Create application
    let mut app = ViewerApp::new(server_address);

    // Initialize window
    app.init_window(event_loop.raw());

    // Run event loop
    event_loop.run_app(&mut app)?;

    info!("Application exited");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_creation() {
        let app = ViewerApp::new("127.0.0.1:8080");
        assert_eq!(app.server_address, "127.0.0.1:8080");
        assert!(app.running);
        assert!(app.display_window.is_none());
    }

    #[test]
    fn test_app_close() {
        let mut app = ViewerApp::new("127.0.0.1:8080");
        app.on_close();
        assert!(!app.running);
    }
}

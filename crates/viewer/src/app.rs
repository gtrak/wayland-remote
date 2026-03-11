//! Viewer application with winit ApplicationHandler
//!
//! This module implements the main application loop using winit's
//! ApplicationHandler trait to manage window lifecycle and frame rendering.

#![cfg(windows)]

use std::sync::mpsc;

use winit::application::ApplicationHandler;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::event::StartCause;
use winit::window::WindowId;

use tracing::{debug, info};

use crate::display::DisplayWindow;
use crate::network::Frame;

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
        // Process any pending frames
        self.process_frames();
    }

    fn window_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: winit::event::WindowEvent,
    ) {
        match event {
            winit::event::WindowEvent::CloseRequested => {
                info!("Window closed, shutting down");
                // Exit the event loop
                _event_loop.exit();
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

    // Run event loop (window will be created in resumed())
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
        assert!(app.display_window.is_none());
    }
}

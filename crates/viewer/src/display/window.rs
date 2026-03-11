//! Window wrapper for winit integration
//!
//! This module provides a DisplayWindow struct that wraps winit's Window
//! and integrates with the GDI renderer for frame display.

use std::ptr;

use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::window::{Window, WindowButtons, WindowLevel};
use winit::dpi::PhysicalSize;
use winit::event_loop::ActiveEventLoop;

use winapi::shared::windef::HDC;
use winapi::um::wingdi::{GetDC, ReleaseDC};

use crate::display::gdi::GdiRenderer;
use crate::network::Frame;

/// Window wrapper that manages display and rendering
///
/// This struct wraps a winit Window and provides GDI-based rendering
/// of frames received from the remote compositor.
pub struct DisplayWindow {
    /// The winit window
    window: Window,
    /// GDI renderer for frame display
    renderer: GdiRenderer,
    /// Whether a redraw has been requested
    redraw_requested: bool,
}

impl DisplayWindow {
    /// Create a new display window with the given title and initial size
    ///
    /// # Arguments
    /// * `event_loop` - The winit event loop to create the window with
    /// * `title` - Window title
    /// * `width` - Initial window width in pixels
    /// * `height` - Initial window height in pixels
    ///
    /// # Returns
    /// A new DisplayWindow instance
    pub fn new(event_loop: &ActiveEventLoop, title: &str, width: u32, height: u32) -> Self {
        // Create window attributes with initial size
        let window_attrs = Window::default_attributes()
            .with_title(title)
            .with_inner_size(PhysicalSize::new(width, height))
            .with_window_button(WindowButtons::DEFAULT)
            .with_window_level(WindowLevel::Normal);

        let window = event_loop
            .create_window(window_attrs)
            .expect("Failed to create window");

        Self {
            window,
            renderer: GdiRenderer::new(),
            redraw_requested: false,
        }
    }

    /// Submit a new frame for display
    ///
    /// The frame is converted to BGRA and stored in the back buffer.
    /// A redraw is requested to display the new frame.
    ///
    /// # Arguments
    /// * `frame` - The frame to display
    pub fn submit_frame(&mut self, frame: &Frame) {
        // Submit frame to renderer
        self.renderer.submit_frame(frame);

        // Request a redraw
        self.window.request_redraw();
    }

    /// Handle window paint event
    ///
    /// Renders the front buffer to the window using GDI.
    pub fn on_paint(&self) {
        // Get the window's device context
        let hwnd = self.window.id().as_raw() as *mut winapi::ctypes::c_void;
        
        unsafe {
            let hdc = GetDC(hwnd);
            if !hdc.is_null() {
                // Get window client area size
                let size = self.window.inner_size();
                
                // Render the front buffer
                self.renderer.render(hdc, size.width as i32, size.height as i32);
                
                ReleaseDC(hwnd, hdc);
            }
        }
    }

    /// Get the current window dimensions
    pub fn inner_size(&self) -> PhysicalSize<u32> {
        self.window.inner_size()
    }

    /// Get a reference to the underlying winit window
    pub fn window(&self) -> &Window {
        &self.window
    }

    /// Get the renderer's current dimensions (last submitted frame)
    pub fn frame_dimensions(&self) -> (u32, u32) {
        self.renderer.dimensions()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_window_compiles() {
        // This test just verifies the struct compiles correctly
        // We can't actually create a window in tests without an event loop
        assert!(true);
    }
}

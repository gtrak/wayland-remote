//! Window wrapper for winit integration
//!
//! This module provides a DisplayWindow struct that wraps winit's Window
//! and integrates with the GDI renderer for frame display.

use std::ptr;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::ActiveEventLoop;
use winit::window::{Window, WindowButtons, WindowLevel};

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
    /// Last resized dimensions for threshold-based resizing
    last_resized_width: Option<u32>,
    last_resized_height: Option<u32>,
}

impl DisplayWindow {
    /// Create a new display window with the given title, size, and optional position
    ///
    /// # Arguments
    /// * `event_loop` - The winit event loop to create the window with
    /// * `title` - Window title
    /// * `width` - Initial window width in pixels
    /// * `height` - Initial window height in pixels
    /// * `x` - Optional window X position (None for default)
    /// * `y` - Optional window Y position (None for default)
    ///
    /// # Returns
    /// A new DisplayWindow instance
    pub fn new(
        event_loop: &ActiveEventLoop,
        title: &str,
        width: u32,
        height: u32,
        x: Option<i32>,
        y: Option<i32>,
    ) -> Self {
        // Create window attributes with initial size
        let mut window_attrs = Window::default_attributes()
            .with_title(title)
            .with_inner_size(PhysicalSize::new(width, height))
            .with_window_button(WindowButtons::DEFAULT)
            .with_window_level(WindowLevel::Normal);

        // Set position if provided
        if let (Some(x), Some(y)) = (x, y) {
            window_attrs = window_attrs.with_position(winit::dpi::PhysicalPosition::new(x, y));
        }
        let window = event_loop
            .create_window(window_attrs)
            .expect("Failed to create window");

        Self {
            window,
            renderer: GdiRenderer::new(),
            last_resized_width: None,
            last_resized_height: None,
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
        // Resize window to match frame dimensions if significantly different
        // Uses threshold-based resizing to prevent flickering from minor dimension changes
        let current_size = self.window.inner_size();
        let new_width = frame.header.width;
        let new_height = frame.header.height;

        // Only resize if this is the first frame or dimensions changed by more than 10%
        let should_resize = match (self.last_resized_width, self.last_resized_height) {
            (None, None) => true, // First frame - always resize
            (Some(last_w), Some(last_h)) => {
                // Calculate percentage change for each dimension
                let width_change =
                    ((new_width as i32 - last_w as i32).abs() as f64 / last_w as f64) * 100.0;
                let height_change =
                    ((new_height as i32 - last_h as i32).abs() as f64 / last_h as f64) * 100.0;
                width_change > 10.0 || height_change > 10.0
            }
            _ => true, // Fallback - resize if one dimension is missing
        };

        if should_resize && (current_size.width != new_width || current_size.height != new_height) {
            self.window
                .set_inner_size(PhysicalSize::new(new_width, new_height));
            self.last_resized_width = Some(new_width);
            self.last_resized_height = Some(new_height);
        }

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
                self.renderer
                    .render(hdc, size.width as i32, size.height as i32);

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

    /// Handle window resize event
    ///
    /// This method is called when the window is resized by the user.
    /// The window will automatically redraw, and the renderer will scale
    /// the frame to fit the new dimensions while preserving aspect ratio.
    ///
    /// # Arguments
    /// * `width` - New window width in pixels
    /// * `height` - New window height in pixels
    /// Parameters accepted for API consistency but current window size
    /// is obtained directly from winit when rendering
    pub fn handle_resize(&self, _width: u32, _height: u32) {
        // The renderer automatically handles aspect ratio preservation
        // via StretchDIBits in the render() method.
        // Just request a redraw to update the display.
        self.window.request_redraw();
    }
}

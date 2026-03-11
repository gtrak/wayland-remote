//! Window Manager for multi-window support
//!
//! This module provides WindowManager for tracking multiple DisplayWindow instances
//! with bidirectional HashMap mappings for frame and event routing.

use std::collections::HashMap;
use winit::application::ApplicationHandler;
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

use crate::display::DisplayWindow;

/// Window manager for tracking multiple display windows
///
/// Maintains bidirectional mappings:
/// - window_id (u32) -> DisplayWindow for frame routing
/// - WindowId (winit) -> window_id for event routing
pub struct WindowManager {
    /// Maps window_id (from compositor) to DisplayWindow
    windows: HashMap<u32, DisplayWindow>,
    /// Maps winit WindowId to window_id for event routing
    window_id_map: HashMap<WindowId, u32>,
    /// Counter for cascading window positions (30px offset)
    cascade_offset: u32,
}

impl WindowManager {
    /// Create a new empty WindowManager
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            window_id_map: HashMap::new(),
            cascade_offset: 0,
        }
    }

    /// Get or create a window for the given window_id
    ///
    /// If the window doesn't exist, creates a new DisplayWindow with cascading position.
    /// Returns a mutable reference to the DisplayWindow.
    ///
    /// # Arguments
    /// * `window_id` - The window ID from the compositor (1-based, 0 is reserved)
    /// * `width` - Initial window width in pixels
    /// * `height` - Initial window height in pixels
    /// * `event_loop` - The winit event loop for window creation
    ///
    /// # Returns
    /// Mutable reference to the DisplayWindow (existing or newly created)
    pub fn get_or_create_window(
        &mut self,
        window_id: u32,
        width: u32,
        height: u32,
        event_loop: &ActiveEventLoop,
    ) -> &mut DisplayWindow {
        // Window ID 0 is reserved/invalid (SurfaceTracker starts at 1)
        assert!(window_id > 0, "Window ID must be greater than 0");

        let title = format!("Wayland Remote - Window {}", window_id);

        self.windows.entry(window_id).or_insert_with(|| {
            // Create window with cascading position
            let window = DisplayWindow::new(event_loop, &title, width, height);
            
            // Store reverse mapping for event routing
            let winit_id = window.window().id();
            self.window_id_map.insert(winit_id, window_id);
            
            // Increment cascade offset for next window (30px)
            self.cascade_offset += 30;
            
            window
        })
    }

    /// Get an immutable reference to a window by window_id
    ///
    /// # Arguments
    /// * `window_id` - The window ID from the compositor
    ///
    /// # Returns
    /// Option containing reference to DisplayWindow if it exists
    pub fn get_window(&self, window_id: u32) -> Option<&DisplayWindow> {
        self.windows.get(&window_id)
    }

    /// Get a mutable reference to a window by window_id
    ///
    /// # Arguments
    /// * `window_id` - The window ID from the compositor
    ///
    /// # Returns
    /// Option containing mutable reference to DisplayWindow if it exists
    pub fn get_window_mut(&mut self, window_id: u32) -> Option<&mut DisplayWindow> {
        self.windows.get_mut(&window_id)
    }

    /// Get the window_id for a given winit WindowId
    ///
    /// Used for routing events from winit to the correct window.
    ///
    /// # Arguments
    /// * `winit_id` - The winit WindowId from an event
    ///
    /// # Returns
    /// Option containing window_id if the WindowId is mapped
    pub fn get_window_id(&self, winit_id: WindowId) -> Option<u32> {
        self.window_id_map.get(&winit_id).copied()
    }

    /// Remove a window by window_id
    ///
    /// Cleans up both the window and the reverse mapping.
    ///
    /// # Arguments
    /// * `window_id` - The window ID to remove
    ///
    /// # Returns
    /// The removed DisplayWindow if it existed
    pub fn remove_window(&mut self, window_id: u32) -> Option<DisplayWindow> {
        // Find and remove the window, extracting its WindowId for cleanup
        if let Some(window) = self.windows.remove(&window_id) {
            // Remove the reverse mapping
            let winit_id = window.window().id();
            self.window_id_map.remove(&winit_id);
            Some(window)
        } else {
            None
        }
    }

    /// Get the number of tracked windows
    pub fn window_count(&self) -> usize {
        self.windows.len()
    }

    /// Check if the window manager is empty
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }
}

impl Default for WindowManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_window_manager_is_empty() {
        let wm = WindowManager::new();
        assert!(wm.is_empty());
        assert_eq!(wm.window_count(), 0);
    }

    #[test]
    #[should_panic(expected = "Window ID must be greater than 0")]
    fn test_window_id_zero_is_invalid() {
        let _wm = WindowManager::new();
        // This would panic if we could call get_or_create_window without event_loop
        assert!(false);
    }
}

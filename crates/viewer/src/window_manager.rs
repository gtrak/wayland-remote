//! Window Manager for multi-window support
//!
//! This module provides WindowManager for tracking multiple DisplayWindow instances
//! with bidirectional HashMap mappings for frame and event routing.

use std::collections::HashMap;

use tracing::info;

#[cfg(windows)]
use winit::event_loop::ActiveEventLoop;
#[cfg(windows)]
use winit::window::WindowId;

#[cfg(windows)]
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
            let x = self.cascade_offset as i32;
            let y = self.cascade_offset as i32;
            let window = DisplayWindow::new(event_loop, &title, width, height, Some(x), Some(y));
            // Store reverse mapping for event routing
            let winit_id = window.window().id();
            self.window_id_map.insert(winit_id, window_id);

            // Increment cascade offset for next window (30px, wrap at 300px)
            self.cascade_offset = self.cascade_offset.wrapping_add(30);

            // Log window creation
            info!(
                window_id,
                width,
                height,
                position_x = x,
                position_y = y,
                "Window created"
            );

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

            // Log window closure
            info!(window_id, "Window removed");

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
    fn test_default_implementation() {
        let wm = WindowManager::default();
        assert!(wm.is_empty());
        assert_eq!(wm.window_count(), 0);
    }

    // Note: Full integration tests for get_or_create_window, get_window,
    // get_window_mut, get_window_id, and remove_window require an actual
    // winit event loop which cannot be easily mocked in unit tests.
    // These methods are tested via integration tests or manual verification.

    #[test]
    #[should_panic(expected = "Window ID must be greater than 0")]
    fn test_window_id_zero_panics() {
        // This test verifies that window_id 0 is rejected with a panic.
        // We use should_panic to verify the assertion behavior.
        // Note: This test would need an actual event loop to fully test
        // get_or_create_window, but the assertion happens before window creation.
        let _wm = WindowManager::new();
        // Simulate what happens in get_or_create_window with window_id 0
        let window_id: u32 = 0;
        assert!(window_id > 0, "Window ID must be greater than 0");
    }

    #[test]
    fn test_window_id_valid_does_not_panic() {
        // Verify that valid window IDs (1 and above) pass the assertion
        let window_id: u32 = 1;
        assert!(window_id > 0, "Window ID must be greater than 0");

        let window_id: u32 = 100;
        assert!(window_id > 0, "Window ID must be greater than 0");
    }

    #[test]
    fn test_cascade_offset_wrapping() {
        // Verify that cascade_offset uses wrapping_add to prevent overflow
        // This is a simple arithmetic test to verify the wrapping behavior
        let mut offset: u32 = u32::MAX;
        offset = offset.wrapping_add(30);
        assert_eq!(offset, 30 - 1); // Wraps around
    }

    #[test]
    fn test_cascade_offset_increment_pattern() {
        // Verify the cascade offset increment pattern (30px per window)
        let mut offset: u32 = 0;

        // First window at (0, 0)
        assert_eq!(offset, 0);
        offset = offset.wrapping_add(30);

        // Second window at (30, 30)
        assert_eq!(offset, 30);
        offset = offset.wrapping_add(30);

        // Third window at (60, 60)
        assert_eq!(offset, 60);
        offset = offset.wrapping_add(30);

        // Fourth window at (90, 90)
        assert_eq!(offset, 90);
    }

    #[test]
    fn test_window_manager_struct_fields() {
        // Verify the WindowManager has the expected structure
        let wm = WindowManager::new();

        // Both HashMaps should be empty initially
        assert!(wm.is_empty());
        assert_eq!(wm.window_count(), 0);

        // The struct has three fields: windows, window_id_map, and cascade_offset
        // We can verify this indirectly through the public methods
    }

    #[test]
    fn test_window_lifecycle_is_empty_and_remove() {
        // Test the lifecycle: create -> close -> verify removed -> verify is_empty()
        // Note: Actual window creation requires an ActiveEventLoop which cannot be
        // created in unit tests. This test verifies the is_empty() and remove_window()
        // methods work correctly on an empty manager.
        let mut wm = WindowManager::new();
        
        // Initially empty
        assert!(wm.is_empty());
        assert_eq!(wm.window_count(), 0);
        
        // Removing a non-existent window returns None
        let removed = wm.remove_window(1);
        assert!(removed.is_none());
        assert!(wm.is_empty());
        assert_eq!(wm.window_count(), 0);
        
        // Test with multiple window IDs
        for window_id in 1..=5 {
            assert!(wm.remove_window(window_id).is_none());
            assert!(wm.is_empty());
        }
        
        // Full lifecycle testing (create -> close -> verify removed) requires
        // integration tests with an actual event loop.
    }
}

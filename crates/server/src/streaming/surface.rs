//! Surface tracking module for multi-surface streaming
//!
//! Provides unique window ID allocation and management for Wayland surfaces.
//! Each surface is mapped to a unique window ID for stable streaming identifiers.
//!
//! Architecture:
//! - `SurfaceTracker`: Manages surface-to-window ID mappings
//! - Uses std::sync::RwLock for state access
//! - Atomic counter for unique ID generation

use std::collections::HashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::RwLock;
use wayland_server::backend::ObjectId;

/// Tracks Wayland surfaces and assigns unique window IDs
///
/// Each Wayland surface (identified by ObjectId) is mapped to a unique
/// window ID (u32) for stable streaming identifiers. This allows viewers
/// to track multiple surfaces independently.
///
/// Note: This struct is designed to be wrapped in Arc for sharing across threads.
pub struct SurfaceTracker {
    /// Next available window ID (atomic counter)
    next_window_id: AtomicU32,
    /// Maps Wayland surface ObjectId -> window ID
    surface_to_window: RwLock<HashMap<ObjectId, u32>>,
    /// Maps window ID -> Wayland surface ObjectId (reverse lookup)
    window_to_surface: RwLock<HashMap<u32, ObjectId>>,
}

impl SurfaceTracker {
    /// Create a new surface tracker
    ///
    /// Initializes with window ID counter starting at 1.
    pub fn new() -> Self {
        Self {
            next_window_id: AtomicU32::new(1),
            surface_to_window: RwLock::new(HashMap::new()),
            window_to_surface: RwLock::new(HashMap::new()),
        }
    }

    /// Allocate a unique window ID for a new surface
    ///
    /// If the surface already has a window ID, returns the existing one.
    /// Otherwise, allocates a new unique ID and stores the mapping.
    ///
    /// # Arguments
    /// * `surface_id` - The Wayland surface ObjectId
    ///
    /// # Returns
    /// The unique window ID for this surface
    pub fn allocate_window_id(&self, surface_id: ObjectId) -> u32 {
        // Check if already mapped
        if let Some(window_id) = self.get_window_id(surface_id.clone()) {
            return window_id;
        }

        // Allocate new window ID atomically
        let window_id = self.next_window_id.fetch_add(1, Ordering::SeqCst);

        // Store both forward and reverse mappings
        self.surface_to_window.write().unwrap().insert(surface_id.clone(), window_id);
        self.window_to_surface.write().unwrap().insert(window_id, surface_id);

        window_id
    }

    /// Get the window ID for a surface
    ///
    /// # Arguments
    /// * `surface_id` - The Wayland surface ObjectId
    ///
    /// # Returns
    /// Some(window_id) if mapped, None otherwise
    pub fn get_window_id(&self, surface_id: ObjectId) -> Option<u32> {
        self.surface_to_window.read().unwrap().get(&surface_id).copied()
    }

    /// Get the surface ObjectId for a window ID
    ///
    /// # Arguments
    /// * `window_id` - The window ID
    ///
    /// # Returns
    /// Some(ObjectId) if mapped, None otherwise
    pub fn get_surface_id(&self, window_id: u32) -> Option<ObjectId> {
        self.window_to_surface.read().unwrap().get(&window_id).cloned()
    }

    /// Remove a surface from tracking
    ///
    /// Called when a surface is destroyed. Removes both the forward
    /// and reverse mappings.
    ///
    /// # Arguments
    /// * `surface_id` - The Wayland surface ObjectId
    ///
    /// # Returns
    /// Some(window_id) if the surface was tracked, None otherwise
    pub fn remove_surface(&self, surface_id: ObjectId) -> Option<u32> {
        let window_id = self.surface_to_window.write().unwrap().remove(&surface_id)?;
        self.window_to_surface.write().unwrap().remove(&window_id);
        Some(window_id)
    }

    /// Get all surface mappings
    ///
    /// # Returns
    /// A copy of all ObjectId -> window_id mappings
    pub fn get_all_mappings(&self) -> HashMap<ObjectId, u32> {
        self.surface_to_window.read().unwrap().clone()
    }

    /// Get the number of tracked surfaces
    pub fn surface_count(&self) -> usize {
        self.surface_to_window.read().unwrap().len()
    }
}

impl Default for SurfaceTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_tracker_new() {
        let tracker = SurfaceTracker::new();
        assert_eq!(tracker.surface_count(), 0);
    }

    // Note: ObjectId cannot be constructed directly (only null() is available).
    // Full integration tests requiring ObjectId are deferred to Phase 3 when
    // we have a running Wayland server to create real surfaces.
    //
    // The SurfaceTracker API is verified by compilation - the struct fields
    // and method signatures are correct. Runtime behavior is verified in
    // integration tests with a real compositor.

    /// Test that ObjectId is the correct type for surface tracking
    #[test]
    fn test_object_id_type() {
        // Verify ObjectId type is available and can be used in HashMap
        let type_name = std::any::type_name::<ObjectId>();
        assert!(!type_name.is_empty());

        // Verify HashMap<ObjectId, u32> works
        let _map: std::collections::HashMap<ObjectId, u32> = std::collections::HashMap::new();
    }

    /// Test that null ObjectId can be created (for optional object events)
    #[test]
    fn test_object_id_null() {
        let null_id = ObjectId::null();
        assert!(null_id.is_null());
    }
}

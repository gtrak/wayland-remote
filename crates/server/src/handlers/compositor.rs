//! Compositor handler for surface lifecycle tracking
//!
//! This module provides the CompositorHandler trait implementation
//! for surface lifecycle tracking:
//! - Surface creation
//! - Buffer attachments via SurfaceAttributes
//! - Surface commits
//! - Surface destruction

// Re-export the CompositorHandler trait for use in state.rs
pub use smithay::wayland::compositor::CompositorHandler;

// The actual implementation is in state.rs to have access to ServerState

//! Display module for Windows window management and rendering
//!
//! This module provides window creation and GDI-based frame rendering
//! for the Wayland remote viewer.

#[cfg(windows)]
pub mod gdi;
#[cfg(windows)]
pub mod window;

#[cfg(windows)]
pub use gdi::GdiRenderer;
#[cfg(windows)]
pub use window::DisplayWindow;

//! Handler modules for Wayland protocol globals
//! Handler modules for Wayland protocol globals
//!
//! This module organizes handlers for different Wayland protocol globals:
//! - compositor: wl_compositor for surface management
//! - seat: wl_seat for input device advertisement
//! - output: wl_output for display information

pub mod compositor;
pub mod seat;
pub mod output;

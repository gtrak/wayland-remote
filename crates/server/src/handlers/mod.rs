//! Handler modules for Wayland protocol globals
//!
//! This module organizes handlers for different Wayland protocol globals:
//! - seat: wl_seat for input device advertisement
//! - output: wl_output for display information

pub mod seat;
pub mod output;

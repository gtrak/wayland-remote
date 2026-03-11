//! Wayland Remote Viewer
//!
//! Windows viewer application for displaying remote Wayland surfaces.

#[cfg(windows)]
pub mod app;
#[cfg(windows)]
pub mod display;
pub mod network;

#[cfg(windows)]
pub mod window_manager;

#[cfg(windows)]
pub use display::{DisplayWindow, GdiRenderer};
pub use network::{Frame, FrameHeader, NetworkError, TcpClient};

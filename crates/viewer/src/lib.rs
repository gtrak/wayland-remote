//! Wayland Remote Viewer
//!
//! Windows viewer application for displaying remote Wayland surfaces.

pub mod network;

pub use network::{Frame, FrameHeader, NetworkError, TcpClient};

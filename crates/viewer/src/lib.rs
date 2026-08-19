//! Viewer library for wayland-remote (Windows).
//!
//! Scaffold for plan 001 issue 01. The QUIC client, Win32 window, and GDI
//! StretchDIBits blit path land in later issues (04-05).

/// Returns the crate version.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

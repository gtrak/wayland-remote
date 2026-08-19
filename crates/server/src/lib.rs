//! Server library for wayland-remote.
//!
//! Shared logic lives in a library so integration tests (see `tests/`) can
//! import it. The headless Wayland compositor and QUIC frame server land in
//! plan 001 issue 03.

/// Returns the crate version.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

//! Wire-format contract for wayland-remote.
//!
//! This crate is pure — no I/O, no runtime — and defines the single shared
//! definition of the wire protocol between the Linux compositor (server) and
//! the Windows viewer:
//!
//! - **Control stream**: a bidirectional QUIC stream carrying protocol
//!   messages (handshake/certificate exchange, surface management, input
//!   events, flow control).
//! - **Frame streams**: per-frame unidirectional QUIC streams. The receiver
//!   skips stale frames (STOP_SENDING on older streams) so a frame never
//!   waits on its predecessor.
//! - **Compression**: frames are lz4 block-compressed; the wire carries a
//!   compression-algorithm field so alternatives (e.g. zstd) can be added
//!   without protocol breakage.
//! - **Pixel format**: BGRA (little-endian ARGB in memory) — pixman's native
//!   readback layout and GDI's 32bpp StretchDIBits layout, so no pixel
//!   conversion anywhere. Row stride travels in the frame header because the
//!   renderer may pad rows.
//!
//! The public API is filled in by plan 001 issue 02; for now the module
//! skeleton below marks the planned shape of the crate.

/// Control-stream message types.
pub mod message;

/// Binary (de)serialization of the wire format.
pub mod codec;

/// Per-frame lz4 block compression helpers.
pub mod compress;

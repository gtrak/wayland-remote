//! Network module for TCP frame streaming
//!
//! This module provides async TCP client functionality for connecting to the
//! Linux server and receiving frame data via a custom binary protocol.
//!
//! # Protocol Format
//!
//! Each frame consists of:
//! - 20-byte header (big-endian)
//!   - window_id: u32 (4 bytes)
//!   - width: u32 (4 bytes)
//!   - height: u32 (4 bytes)
//!   - timestamp: u64 (8 bytes)
//! - Variable-length payload (width * height * 4 bytes, RGBA format)

pub mod client;
pub mod protocol;

pub use client::TcpClient;
pub use protocol::{Frame, FrameHeader};

/// Error types for network operations
#[derive(Debug, thiserror::Error)]
pub enum NetworkError {
    #[error("Connection error: {0}")]
    Connection(String),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frame_header_wire_size() {
        // Wire format should be exactly 20 bytes: 4 + 4 + 4 + 8
        assert_eq!(
            FrameHeader::SIZE,
            20,
            "Wire format must be exactly 20 bytes"
        );
    }

    #[test]
    fn test_frame_struct_exists() {
        // Verify Frame struct can be instantiated
        let frame = Frame {
            header: FrameHeader {
                window_id: 1,
                width: 1920,
                height: 1080,
                timestamp: 1234567890,
            },
            data: vec![0u8; 1920 * 1080 * 4],
        };
        assert_eq!(frame.header.width, 1920);
        assert_eq!(frame.header.height, 1080);
        assert_eq!(frame.data.len(), 1920 * 1080 * 4);
    }
}

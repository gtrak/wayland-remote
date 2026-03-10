//! Binary frame protocol for TCP streaming
//!
//! Defines the 20-byte header format and encoding/decoding functions
//! for streaming RGBA frames from the compositor to viewers.
//!
//! Header format (20 bytes, big-endian):
//! - window_id: u32 (4 bytes) - Unique surface identifier
//! - width: u32 (4 bytes) - Frame width in pixels
//! - height: u32 (4 bytes) - Frame height in pixels
//! - timestamp_us: u64 (8 bytes) - Unix timestamp in microseconds
//!
//! Payload: Raw RGBA bytes (width × height × 4 bytes)

use bytes::{BufMut, BytesMut};

/// Frame header for binary protocol
///
/// Contains metadata about a frame before the RGBA payload.
/// Total size: 20 bytes (4 + 4 + 4 + 8)
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameHeader {
    /// Unique window/surface identifier
    pub window_id: u32,
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Unix timestamp in microseconds
    pub timestamp_us: u64,
}

impl FrameHeader {
    /// Size of the header in bytes
    pub const SIZE: usize = 20; // 4 + 4 + 4 + 8

    /// Create a new frame header
    pub fn new(window_id: u32, width: u32, height: u32, timestamp_us: u64) -> Self {
        Self {
            window_id,
            width,
            height,
            timestamp_us,
        }
    }

    /// Calculate the total payload size for this frame (RGBA bytes)
    pub fn payload_size(&self) -> usize {
        (self.width as usize) * (self.height as usize) * 4
    }

    /// Encode header into a buffer (big-endian)
    ///
    /// Writes the 20-byte header to the provided BytesMut buffer.
    pub fn encode(&self, buf: &mut BytesMut) {
        buf.put_u32(self.window_id);
        buf.put_u32(self.width);
        buf.put_u32(self.height);
        buf.put_u64(self.timestamp_us);
    }

    /// Decode header from a buffer (big-endian)
    ///
    /// Parses a 20-byte header from the provided buffer.
    /// Returns None if buffer is too small.
    pub fn decode(buf: &[u8]) -> Option<Self> {
        if buf.len() < Self::SIZE {
            return None;
        }

        Some(Self {
            window_id: u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]),
            width: u32::from_be_bytes([buf[4], buf[5], buf[6], buf[7]]),
            height: u32::from_be_bytes([buf[8], buf[9], buf[10], buf[11]]),
            timestamp_us: u64::from_be_bytes([
                buf[12], buf[13], buf[14], buf[15],
                buf[16], buf[17], buf[18], buf[19],
            ]),
        })
    }
}

/// Encode a complete frame (header + RGBA payload)
///
/// Creates a BytesMut buffer containing the frame header followed by
/// the raw RGBA pixel data.
///
/// # Arguments
/// * `header` - Frame metadata (window_id, dimensions, timestamp)
/// * `rgba_data` - Raw RGBA pixel bytes (width × height × 4)
///
/// # Returns
/// BytesMut containing header (20 bytes) + payload
pub fn encode_frame(header: &FrameHeader, rgba_data: &[u8]) -> BytesMut {
    let mut buf = BytesMut::with_capacity(FrameHeader::SIZE + rgba_data.len());
    header.encode(&mut buf);
    buf.put_slice(rgba_data);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_size() {
        assert_eq!(FrameHeader::SIZE, 20);
    }

    #[test]
    fn test_header_encode_decode() {
        let header = FrameHeader::new(0xDEADBEEF, 1920, 1080, 1234567890);
        
        let mut buf = BytesMut::with_capacity(FrameHeader::SIZE);
        header.encode(&mut buf);
        
        assert_eq!(buf.len(), FrameHeader::SIZE);
        
        let decoded = FrameHeader::decode(&buf).expect("Failed to decode header");
        assert_eq!(header, decoded);
    }

    #[test]
    fn test_header_decode_insufficient_data() {
        let buf = vec![0u8; 10]; // Less than 20 bytes
        assert!(FrameHeader::decode(&buf).is_none());
    }

    #[test]
    fn test_payload_size() {
        let header = FrameHeader::new(1, 1920, 1080, 0);
        assert_eq!(header.payload_size(), 1920 * 1080 * 4); // 8,294,400 bytes
    }

    #[test]
    fn test_encode_frame() {
        let header = FrameHeader::new(42, 100, 100, 1000000);
        let rgba_data = vec![0xFFu8; 100 * 100 * 4]; // 100x100 RGBA
        
        let frame = encode_frame(&header, &rgba_data);
        
        // Header + payload
        assert_eq!(frame.len(), FrameHeader::SIZE + 100 * 100 * 4);
        
        // Verify header can be decoded
        let decoded = FrameHeader::decode(&frame[..FrameHeader::SIZE])
            .expect("Failed to decode header");
        assert_eq!(header, decoded);
    }

    #[test]
    fn test_big_endian_byte_order() {
        let header = FrameHeader::new(0x01020304, 0x05060708, 0x090A0B0C, 0x0D0E0F1011121314);
        let mut buf = BytesMut::with_capacity(FrameHeader::SIZE);
        header.encode(&mut buf);
        
        // Verify big-endian byte order
        assert_eq!(&buf[0..4], &[0x01, 0x02, 0x03, 0x04]);
        assert_eq!(&buf[4..8], &[0x05, 0x06, 0x07, 0x08]);
        assert_eq!(&buf[8..12], &[0x09, 0x0A, 0x0B, 0x0C]);
        assert_eq!(&buf[12..20], &[0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14]);
    }
}

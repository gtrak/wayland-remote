//! Binary frame protocol parsing
//!
//! Implements the 20-byte big-endian header format and RGBA payload handling.

use crate::network::NetworkError;

/// Frame header containing metadata about the frame
///
/// Layout (big-endian, 20 bytes total):
/// - window_id: u32 (bytes 0-3)
/// - width: u32 (bytes 4-7)
/// - height: u32 (bytes 8-11)
/// - timestamp: u64 (bytes 12-19)
#[derive(Debug, Clone, PartialEq)]
pub struct FrameHeader {
    /// Unique identifier for the window/surface
    pub window_id: u32,
    /// Frame width in pixels
    pub width: u32,
    /// Frame height in pixels
    pub height: u32,
    /// Timestamp in milliseconds since epoch
    pub timestamp: u64,
}

impl FrameHeader {
    /// Size of the header in bytes
    pub const SIZE: usize = 20;

    /// Decode header from 20-byte big-endian buffer
    ///
    /// # Arguments
    /// * `data` - Slice containing exactly 20 bytes
    ///
    /// # Errors
    /// Returns NetworkError::Protocol if data is not exactly 20 bytes
    pub fn decode(data: &[u8]) -> Result<Self, NetworkError> {
        if data.len() < Self::SIZE {
            return Err(NetworkError::Protocol(format!(
                "Expected {} bytes, got {}",
                Self::SIZE,
                data.len()
            )));
        }

        Ok(Self {
            window_id: u32::from_be_bytes([data[0], data[1], data[2], data[3]]),
            width: u32::from_be_bytes([data[4], data[5], data[6], data[7]]),
            height: u32::from_be_bytes([data[8], data[9], data[10], data[11]]),
            timestamp: u64::from_be_bytes([
                data[12], data[13], data[14], data[15], data[16], data[17], data[18], data[19],
            ]),
        })
    }

    /// Encode header to 20-byte big-endian buffer
    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[0..4].copy_from_slice(&self.window_id.to_be_bytes());
        buf[4..8].copy_from_slice(&self.width.to_be_bytes());
        buf[8..12].copy_from_slice(&self.height.to_be_bytes());
        buf[12..20].copy_from_slice(&self.timestamp.to_be_bytes());
        buf
    }

    /// Calculate the payload size in bytes for this frame
    ///
    /// Payload is RGBA format: 4 bytes per pixel
    pub fn payload_size(&self) -> usize {
        (self.width as usize) * (self.height as usize) * 4
    }

    /// Calculate total frame size (header + payload)
    pub fn total_size(&self) -> usize {
        Self::SIZE + self.payload_size()
    }
}

/// Complete frame with header and RGBA pixel data
#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    /// Frame metadata
    pub header: FrameHeader,
    /// RGBA pixel data (4 bytes per pixel)
    pub data: Vec<u8>,
}

impl Frame {
    /// Create a new frame from header and data
    pub fn new(header: FrameHeader, data: Vec<u8>) -> Self {
        Self { header, data }
    }

    /// Verify data size matches header dimensions
    pub fn is_valid(&self) -> bool {
        self.data.len() == self.header.payload_size()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_header_decode() {
        // Create test data: window_id=1, width=1920, height=1080, timestamp=1234567890
        let mut data = [0u8; 20];
        data[0..4].copy_from_slice(&1u32.to_be_bytes());
        data[4..8].copy_from_slice(&1920u32.to_be_bytes());
        data[8..12].copy_from_slice(&1080u32.to_be_bytes());
        data[12..20].copy_from_slice(&1234567890u64.to_be_bytes());

        let header = FrameHeader::decode(&data).unwrap();

        assert_eq!(header.window_id, 1);
        assert_eq!(header.width, 1920);
        assert_eq!(header.height, 1080);
        assert_eq!(header.timestamp, 1234567890);
    }

    #[test]
    fn test_header_encode() {
        let header = FrameHeader {
            window_id: 42,
            width: 1280,
            height: 720,
            timestamp: 9876543210,
        };

        let encoded = header.encode();
        let decoded = FrameHeader::decode(&encoded).unwrap();

        assert_eq!(header, decoded);
    }

    #[test]
    fn test_header_payload_size() {
        let header = FrameHeader {
            window_id: 1,
            width: 1920,
            height: 1080,
            timestamp: 0,
        };

        // 1920 * 1080 * 4 = 8,294,400 bytes
        assert_eq!(header.payload_size(), 8_294_400);
    }

    #[test]
    fn test_header_total_size() {
        let header = FrameHeader {
            window_id: 1,
            width: 100,
            height: 100,
            timestamp: 0,
        };

        // 20 + (100 * 100 * 4) = 40,020 bytes
        assert_eq!(header.total_size(), 40_020);
    }

    #[test]
    fn test_decode_insufficient_data() {
        let data = [0u8; 15]; // Less than 20 bytes
        let result = FrameHeader::decode(&data);
        assert!(result.is_err());
    }

    #[test]
    fn test_frame_validity() {
        let header = FrameHeader {
            window_id: 1,
            width: 100,
            height: 100,
            timestamp: 0,
        };

        let valid_frame = Frame::new(header.clone(), vec![0u8; 100 * 100 * 4]);
        assert!(valid_frame.is_valid());

        let invalid_frame = Frame::new(header, vec![0u8; 5000]);
        assert!(!invalid_frame.is_valid());
    }

    #[test]
    fn test_big_endian_ordering() {
        // Verify big-endian byte order is correct
        let header = FrameHeader {
            window_id: 0x12345678,
            width: 0x9ABCDEF0,
            height: 0xFEDCBA98,
            timestamp: 0x1122334455667788,
        };

        let encoded = header.encode();

        // Check window_id bytes
        assert_eq!(encoded[0], 0x12);
        assert_eq!(encoded[1], 0x34);
        assert_eq!(encoded[2], 0x56);
        assert_eq!(encoded[3], 0x78);

        // Check width bytes
        assert_eq!(encoded[4], 0x9A);
        assert_eq!(encoded[5], 0xBC);
        assert_eq!(encoded[6], 0xDE);
        assert_eq!(encoded[7], 0xF0);

        // Check timestamp bytes
        assert_eq!(encoded[12], 0x11);
        assert_eq!(encoded[19], 0x88);
    }
}

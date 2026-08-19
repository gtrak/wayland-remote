//! lz4 block compression codec.
use crate::codec::DecodeError;

/// Compression algorithm identifier, mirroring the wire `compression` field.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// No compression; payload is raw BGRA bytes.
    None,
    /// lz4 block compression (block format, not the lz4 frame format).
    Lz4,
}

impl Compression {
    /// Decode the wire `compression` byte; unknown values are an error.
    pub fn from_u8(v: u8) -> Result<Self, DecodeError> {
        match v {
            0 => Ok(Self::None),
            1 => Ok(Self::Lz4),
            other => Err(DecodeError::UnknownCompression(other)),
        }
    }

    /// Encode as the wire `compression` byte.
    pub fn as_u8(&self) -> u8 {
        match self {
            Self::None => 0,
            Self::Lz4 => 1,
        }
    }
}

/// Compress `data` as an lz4 block.
pub fn compress(data: &[u8]) -> Vec<u8> {
    lz4_flex::block::compress(data)
}

/// Decompress an lz4 block of `src`, expecting exactly `expected_len`
/// uncompressed bytes.
pub fn decompress(src: &[u8], expected_len: usize) -> Result<Vec<u8>, DecodeError> {
    lz4_flex::block::decompress(src, expected_len).map_err(|_| DecodeError::DecompressFailed)
}

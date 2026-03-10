//! Pixel export module for RGBA extraction from offscreen buffers
//!
//! This module provides functionality for extracting raw RGBA pixel data
//! from rendered Wayland surface framebuffers using Smithay's ExportMem trait.

use smithay::backend::renderer::{
    pixman::PixmanRenderer,
    ExportMem, Bind,
};
use smithay::backend::allocator::Fourcc;
use smithay::utils::{Rectangle, Point, Size, Buffer as BufferCoord};
use smithay::reexports::pixman::Image;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;

/// RGBA pixel data extracted from a framebuffer
///
/// Contains the raw pixel bytes in ABGR8888 format (ARGB in little-endian)
/// along with the dimensions of the frame.
#[derive(Debug, Clone)]
pub struct RgbaData {
    /// Width of the frame in pixels
    pub width: u32,
    /// Height of the frame in pixels
    pub height: u32,
    /// Raw pixel data in ABGR8888 format (4 bytes per pixel: B, G, R, A)
    pub data: Vec<u8>,
}

impl RgbaData {
    /// Create new RgbaData from pixel buffer
    pub fn new(width: u32, height: u32, data: Vec<u8>) -> Self {
        Self { width, height, data }
    }
    
    /// Get the total number of bytes in the pixel data
    pub fn byte_size(&self) -> usize {
        self.data.len()
    }
    
    /// Get expected byte size for given dimensions
    pub fn expected_size(width: u32, height: u32) -> usize {
        (width * height * 4) as usize
    }
}

/// Extract RGBA pixel data from an offscreen pixman buffer
///
/// This function:
/// 1. Binds the offscreen buffer as a target
/// 2. Uses ExportMem::copy_framebuffer() to copy to a PixmanMapping
/// 3. Calls map_texture() to get a &[u8] slice of the pixel data
/// 4. Clones the bytes into an owned Vec<u8> for transmission
///
/// # Arguments
/// * `renderer` - The PixmanRenderer to use for pixel extraction
/// * `buffer` - The offscreen pixman buffer containing rendered content
///
/// # Returns
/// Some(RgbaData) containing the pixel data and dimensions, or None if extraction fails
pub fn extract_rgba_from_buffer(
    renderer: &mut PixmanRenderer,
    buffer: &mut Image<'static, 'static>,
) -> Option<RgbaData> {
    let width = buffer.width() as u32;
    let height = buffer.height() as u32;
    
    // Bind the offscreen buffer as a target
    let target = renderer.bind(buffer).ok()?;
    
    // Use ExportMem::copy_framebuffer to copy the target to a PixmanMapping
    // This converts the buffer to ABGR8888 format for easy pixel access
    let target_size = Size::from((width as i32, height as i32));
    let target_rect = Rectangle::<i32, BufferCoord>::new(
        Point::<i32, BufferCoord>::default(), 
        target_size
    );
    let mapping = renderer.copy_framebuffer(&target, target_rect, Fourcc::Abgr8888).ok()?;
    
    // Map the texture to get a &[u8] slice of the pixel data
    // The mapping must be held until we clone the data
    let pixel_slice = renderer.map_texture(&mapping).ok()?;
    
    // Clone the bytes into an owned Vec<u8> for transmission
    // We need owned data because the mapping will be dropped
    let data = pixel_slice.to_vec();
    
    // Verify we got the expected amount of data (M-2: reject corrupted frames)
    let expected_size = RgbaData::expected_size(width, height);
    if data.len() != expected_size {
        tracing::error!(
            "Size mismatch: extracted {} bytes, expected {} for {}x{} frame",
            data.len(),
            expected_size,
            width,
            height
        );
        return None; // M-2: Return None to prevent storing corrupted frames
    }
    
    Some(RgbaData::new(width, height, data))
}

/// Extract RGBA pixel data from a rendered Wayland surface via its offscreen buffer
///
/// This function:
/// 1. Gets the offscreen buffer for the surface from ServerState
/// 2. Calls extract_rgba_from_buffer() to extract the pixels
///
/// # Arguments
/// * `renderer` - The PixmanRenderer to use for pixel extraction
/// * `surface` - The Wayland surface to extract pixels from
/// * `offscreen_buffer` - The offscreen buffer for this surface
///
/// # Returns
/// Some(RgbaData) containing the pixel data and dimensions, or None if extraction fails
pub fn extract_rgba_pixels(
    renderer: &mut PixmanRenderer,
    _surface: &WlSurface,
    offscreen_buffer: &mut Image<'static, 'static>,
) -> Option<RgbaData> {
    extract_rgba_from_buffer(renderer, offscreen_buffer)
}

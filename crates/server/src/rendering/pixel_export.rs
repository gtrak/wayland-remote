//! Pixel export module for RGBA extraction from offscreen buffers
//!
//! This module provides functionality for extracting raw RGBA pixel data
//! from rendered Wayland surface framebuffers using Smithay's ExportMem trait.

use smithay::backend::renderer::{
    pixman::PixmanRenderer,
    ExportMem, ImportMemWl, Texture,
};
use smithay::backend::allocator::Fourcc;
use smithay::utils::{Rectangle, Transform};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::{with_states, SurfaceAttributes, BufferAssignment};

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

/// Extract RGBA pixel data from a rendered Wayland surface
///
/// This function:
/// 1. Imports the surface's shared memory buffer to get the texture
/// 2. Uses ExportMem::copy_framebuffer() to copy the texture to a PixmanMapping
/// 3. Calls map_texture() to get a &[u8] slice of the pixel data
/// 4. Clones the bytes into an owned Vec<u8> for transmission
///
/// # Arguments
/// * `renderer` - The PixmanRenderer to use for pixel extraction
/// * `surface` - The Wayland surface to extract pixels from
///
/// # Returns
/// Some(RgbaData) containing the pixel data and dimensions, or None if extraction fails
pub fn extract_rgba_pixels(
    renderer: &mut PixmanRenderer,
    surface: &WlSurface,
) -> Option<RgbaData> {
    // Import the surface's shared memory buffer to get the texture
    let texture: Texture = {
        let result: Result<_, _> = with_states(surface, |surface_data| {
            let mut attrs = surface_data.cached_state.get::<SurfaceAttributes>();
            let buffer = match &attrs.current().buffer {
                Some(BufferAssignment::NewBuffer(buf)) => Some(buf.clone()),
                Some(BufferAssignment::Removed) => None,
                None => None,
            };
            match buffer {
                Some(b) => Ok(renderer.import_shm_buffer(&b, Some(surface_data), &[])?),
                None => Err("No buffer attached to surface".into()),
            }
        });
        
        result.ok()?
    };
    
    // Get dimensions from the texture
    let size = texture.size();
    let width = size.w as u32;
    let height = size.h as u32;
    
    // Use ExportMem::copy_framebuffer to copy the texture to a PixmanMapping
    // This converts the texture to ABGR8888 format for easy pixel access
    let mapping = renderer.copy_framebuffer(&texture, Fourcc::Abgr8888, None).ok()?;
    
    // Map the texture to get a &[u8] slice of the pixel data
    // The mapping must be held until we clone the data
    let pixel_slice = renderer.map_texture(&mapping).ok()?;
    
    // Clone the bytes into an owned Vec<u8> for transmission
    // We need owned data because the mapping will be dropped
    let data = pixel_slice.to_vec();
    
    // Verify we got the expected amount of data
    let expected_size = RgbaData::expected_size(width, height);
    if data.len() != expected_size {
        tracing::warn!(
            "Extracted {} bytes, expected {} for {}x{} frame",
            data.len(),
            expected_size,
            width,
            height
        );
    }
    
    Some(RgbaData::new(width, height, data))
}

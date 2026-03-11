//! GDI renderer using StretchDIBits for frame display
//!
//! This module provides Windows GDI rendering functionality for displaying
//! RGBA frames from the remote compositor. Uses StretchDIBits with proper
//! color conversion and top-down bitmap format.

use std::ptr;
use std::sync::Mutex;

use winapi::shared::minwindef::{DWORD, FALSE};
use winapi::shared::windef::{BITMAPINFO, BITMAPINFOHEADER, HDC, HBITMAP, RECT};
use winapi::um::wingdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, GetDIBits,
    ReleaseDC, SelectObject, StretchDIBits, SRCCOPY, DIB_RGB_COLORS,
};

use crate::network::Frame;

/// GDI renderer for displaying RGBA frames
///
/// Uses double buffering to prevent tearing. The back buffer is where
/// frames are submitted, and the front buffer is what's currently displayed.
pub struct GdiRenderer {
    /// Front buffer bitmap (currently displayed)
    front_buffer: Option<HBITMAP>,
    /// Back buffer bitmap (next frame to display)
    back_buffer: Option<HBITMAP>,
    /// Current frame dimensions
    width: u32,
    height: u32,
}

impl GdiRenderer {
    /// Create a new GDI renderer
    pub fn new() -> Self {
        Self {
            front_buffer: None,
            back_buffer: None,
            width: 0,
            height: 0,
        }
    }

    /// Submit a new frame to the back buffer
    ///
    /// Converts RGBA to BGRA for GDI compatibility and creates a new
    /// device-independent bitmap (DIB).
    ///
    /// # Arguments
    /// * `frame` - The frame to render (RGBA format)
    pub fn submit_frame(&mut self, frame: &Frame) {
        let width = frame.header.width as i32;
        let height = frame.header.height as i32;

        // Convert RGBA to BGRA (Windows expects BGRA order)
        let bgra_data = self.convert_rgba_to_bgra(&frame.data, width as usize, height as usize);

        // Create DIB with the converted data
        let bitmap = self.create_dib(width, height, &bgra_data);

        // Swap buffers: old front becomes back (will be deleted), back becomes front
        if let Some(old_front) = self.front_buffer.take() {
            unsafe {
                DeleteObject(old_front);
            }
        }
        self.front_buffer = self.back_buffer.take();
        self.back_buffer = Some(bitmap);

        // Update dimensions
        self.width = frame.header.width;
        self.height = frame.header.height;
    }

    /// Convert RGBA pixel data to BGRA format
    ///
    /// Windows GDI expects pixels in BGRA order, but our frames are RGBA.
    /// This function swaps the red and blue channels.
    ///
    /// # Arguments
    /// * `rgba_data` - Source data in RGBA format (4 bytes per pixel)
    /// * `width` - Image width in pixels
    /// * `height` - Image height in pixels
    ///
    /// # Returns
    /// Vec<u8> containing BGRA pixel data
    fn convert_rgba_to_bgra(&self, rgba_data: &[u8], width: usize, height: usize) -> Vec<u8> {
        let pixel_count = width * height;
        let mut bgra_data = Vec::with_capacity(rgba_data.len());

        // Process 4 bytes at a time (one pixel)
        for i in 0..pixel_count {
            let offset = i * 4;
            // RGBA: [R, G, B, A]
            // BGRA: [B, G, R, A]
            bgra_data.push(rgba_data[offset + 2]); // B
            bgra_data.push(rgba_data[offset + 1]); // G
            bgra_data.push(rgba_data[offset]);     // R
            bgra_data.push(rgba_data[offset + 3]); // A
        }

        bgra_data
    }

    /// Create a device-independent bitmap from pixel data
    ///
    /// Uses negative biHeight to create a top-down DIB (origin at top-left).
    ///
    /// # Arguments
    /// * `width` - Bitmap width in pixels
    /// * `height` - Bitmap height in pixels
    /// * `data` - Pixel data in BGRA format
    ///
    /// # Returns
    /// Handle to the created bitmap
    fn create_dib(&self, width: i32, height: i32, data: &[u8]) -> HBITMAP {
        unsafe {
            // Create BITMAPINFOHEADER for 32-bit bitmap
            // Negative height creates top-down DIB
            let mut bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as DWORD,
                    biWidth: width,
                    biHeight: -height, // Negative for top-down DIB
                    biPlanes: 1,
                    biBitCount: 32,    // 32 bits per pixel
                    biCompression: 0,  // BI_RGB (no compression)
                    biSizeImage: 0,    // Can be 0 for BI_RGB
                    biXPelsPerMeter: 0,
                    biYPelsPerMeter: 0,
                    biClrUsed: 0,
                    biClrImportant: 0,
                },
                bmiColors: [winapi::shared::windef::RGBQUAD {
                    rgbBlue: 0,
                    rgbGreen: 0,
                    rgbRed: 0,
                    rgbReserved: 0,
                }; 1],
            };

            // Get DC from null window to create DIB section
            let hdc = GetDC(ptr::null_mut());
            if hdc.is_null() {
                panic!("Failed to get DC");
            }

            // Create DIB section
            let mut bits_ptr: *mut winapi::ctypes::c_void = ptr::null_mut();
            let hbitmap = CreateDIBSection(
                hdc,
                &mut bmi,
                DIB_RGB_COLORS,
                &mut bits_ptr,
                0,
                0,
            );

            ReleaseDC(ptr::null_mut(), hdc);

            if hbitmap.is_null() {
                panic!("Failed to create DIB section");
            }

            // Copy our pixel data into the DIB section
            if !bits_ptr.is_null() {
                ptr::copy_nonoverlapping(data.as_ptr(), bits_ptr as *mut u8, data.len());
            }

            hbitmap
        }
    }

    /// Render the front buffer to the given device context
    ///
    /// # Arguments
    /// * `hdc` - Device context to render to
    /// * `dest_width` - Destination width in pixels
    /// * `dest_height` - Destination height in pixels
    pub fn render(&self, hdc: HDC, dest_width: i32, dest_height: i32) {
        if let Some(bitmap) = self.front_buffer {
            unsafe {
                // Create compatible DC for the bitmap
                let hdc_mem = CreateCompatibleDC(hdc);
                if !hdc_mem.is_null() {
                    let old_bitmap = SelectObject(hdc_mem, bitmap as _);

                    // Use StretchDIBits to render with proper scaling
                    let _ = StretchDIBits(
                        hdc,
                        0,
                        0,
                        dest_width,
                        dest_height,
                        0,
                        0,
                        self.width as u32,
                        self.height as u32,
                        ptr::null(),
                        &bitmap as *const _ as *const _,
                        0,
                        SRCCOPY,
                    );

                    // Restore old bitmap and clean up
                    let _ = SelectObject(hdc_mem, old_bitmap as _);
                    DeleteDC(hdc_mem);
                }
            }
        }
    }

    /// Get the current frame dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

impl Default for GdiRenderer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for GdiRenderer {
    fn drop(&mut self) {
        // Clean up GDI objects
        unsafe {
            if let Some(bitmap) = self.front_buffer {
                if !bitmap.is_null() {
                    DeleteObject(bitmap);
                }
            }
            if let Some(bitmap) = self.back_buffer {
                if !bitmap.is_null() {
                    DeleteObject(bitmap);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_renderer_creation() {
        let renderer = GdiRenderer::new();
        assert_eq!(renderer.dimensions(), (0, 0));
    }

    #[test]
    fn test_rgba_to_bgra_conversion() {
        let renderer = GdiRenderer::new();
        
        // Single pixel: RGBA = [255, 128, 64, 200] (red=255, green=128, blue=64, alpha=200)
        let rgba = vec![255, 128, 64, 200];
        let bgra = renderer.convert_rgba_to_bgra(&rgba, 1, 1);
        
        // BGRA should be: [64, 128, 255, 200] (blue=64, green=128, red=255, alpha=200)
        assert_eq!(bgra, vec![64, 128, 255, 200]);
    }

    #[test]
    fn test_rgba_to_bgra_multiple_pixels() {
        let renderer = GdiRenderer::new();
        
        // Two pixels
        let rgba = vec![
            255, 0, 0, 255,   // Red
            0, 255, 0, 255,   // Green
        ];
        let bgra = renderer.convert_rgba_to_bgra(&rgba, 2, 1);
        
        // BGRA: Blue, Green, Red, Alpha
        assert_eq!(bgra, vec![
            0, 0, 255, 255,   // Red in BGRA
            0, 255, 0, 255,   // Green in BGRA (same)
        ]);
    }

    #[test]
    fn test_frame_submission_updates_dimensions() {
        let mut renderer = GdiRenderer::new();
        
        let frame = Frame {
            header: crate::network::FrameHeader {
                window_id: 1,
                width: 800,
                height: 600,
                timestamp: 1234567890,
            },
            data: vec![0u8; 800 * 600 * 4],
        };
        
        renderer.submit_frame(&frame);
        assert_eq!(renderer.dimensions(), (800, 600));
    }
}

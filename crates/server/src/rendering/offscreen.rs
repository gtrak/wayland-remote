//! Offscreen buffer management and surface rendering
//!
//! This module provides functions for:
//! - Creating offscreen pixman buffers for headless rendering
//! - Rendering Wayland surfaces to offscreen buffers
//! - Managing per-surface buffer lifecycle

use smithay::backend::renderer::{
    pixman::PixmanRenderer,
    Offscreen, Bind, Renderer, ImportMemWl, Frame, Texture,
};
use smithay::utils::{Size, Point, Transform, Rectangle};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::backend::allocator::Fourcc;
use smithay::reexports::pixman::Image;
use smithay::utils::{Physical, Buffer as BufferCoord};

/// Create an offscreen buffer for rendering
///
/// Creates a memory-backed pixman buffer for headless rendering.
/// The buffer format is Abgr8888 (ARGB in little-endian).
///
/// # Arguments
/// * `renderer` - The PixmanRenderer to use for buffer creation
/// * `width` - Width of the buffer in pixels
/// * `height` - Height of the buffer in pixels
///
/// # Returns
/// A pixman::Image for rendering, or error if buffer creation fails
pub fn create_offscreen_buffer(
    renderer: &mut PixmanRenderer,
    width: i32,
    height: i32,
) -> Result<Image<'static, 'static>, smithay::backend::renderer::pixman::PixmanError> {
    let size = Size::from((width, height));
    // Create headless memory buffer in ARGB8888 in little-endian (ABGR) format
    renderer.create_buffer(Fourcc::Abgr8888, size)
}

/// Render a Wayland surface to an offscreen buffer
///
/// This function:
/// 1. Imports the surface's shared memory buffer
/// 2. Binds the offscreen target
/// 3. Renders the surface texture to the buffer
/// 4. Finishes the frame
///
/// # Arguments
/// * `renderer` - The PixmanRenderer to use for rendering
/// * `surface` - The Wayland surface to render
/// * `buffer` - The offscreen buffer to render into
///
/// # Returns
/// Ok(()) on successful render, Err otherwise
pub fn render_surface_to_buffer(
    renderer: &mut PixmanRenderer,
    surface: &WlSurface,
    buffer: &mut Image<'static, 'static>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Import the shared memory buffer from the surface
    // We need to do this inside with_states to get the surface_data reference
    let texture = {
        use smithay::wayland::compositor::{with_states, SurfaceAttributes, BufferAssignment};
        let result: Result<_, _> = with_states(surface, |surface_data| {
            let mut attrs = surface_data.cached_state.get::<SurfaceAttributes>();
            // BufferAssignment is an enum: Removed or NewBuffer(WlBuffer)
            let buffer = match &attrs.current().buffer {
                Some(BufferAssignment::NewBuffer(buf)) => Some(buf.clone()),
                Some(BufferAssignment::Removed) => None,
                None => None,
            };
            match buffer {
                Some(b) => Ok::<_, Box<dyn std::error::Error>>(renderer.import_shm_buffer(&b, Some(surface_data), &[])?),
                None => Err("No buffer attached to surface".into()),
            }
        });
        result?
    };

    // Get buffer dimensions for the offscreen target
    let buffer_size = Size::from((buffer.width() as i32, buffer.height() as i32));

    // Bind the offscreen target
    let mut target = renderer.bind(buffer)?;

    // Render to the target (render requires framebuffer, output_size, dst_transform)
    let mut frame = renderer.render(&mut target, buffer_size, Transform::Normal)?;
    
    // Render the texture to the target
    // render_texture_from_to: texture, src, dst, damage, opaque_regions, src_transform, alpha
    frame.render_texture_from_to(
        &texture,
        Rectangle::<f64, BufferCoord>::new(Point::<f64, BufferCoord>::default(), Size::<f64, BufferCoord>::from((texture.width() as f64, texture.height() as f64))),
        Rectangle::<i32, Physical>::new(Point::<i32, Physical>::default(), buffer_size),
        &[],
        &[],
        Transform::Normal,
        1.0,
    )?;

    // Finish the frame
    let _ = frame.finish()?;

    Ok(())
}

/// Try to render a surface to an offscreen buffer with error handling
///
/// This is a wrapper that logs errors instead of propagating them,
/// useful for integration in the commit handler where we don't want
/// to panic on render failures.
///
/// # Arguments
/// * `renderer` - The PixmanRenderer to use for rendering
/// * `surface` - The Wayland surface to render
/// * `buffer` - The offscreen buffer to render into
///
/// # Returns
/// true if rendering succeeded, false otherwise
pub fn try_render_surface_to_buffer(
    renderer: &mut PixmanRenderer,
    surface: &WlSurface,
    buffer: &mut Image<'static, 'static>,
) -> bool {
    render_surface_to_buffer(renderer, surface, buffer).is_ok()
}

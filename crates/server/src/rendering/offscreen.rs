//! Offscreen buffer management and surface rendering
//!
//! This module provides functions for:
//! - Creating offscreen pixman buffers for headless rendering
//! - Rendering Wayland surfaces to offscreen buffers
//! - Managing per-surface buffer lifecycle

use smithay::backend::renderer::{
    pixman::PixmanRenderer,
    Offscreen, Bind, Renderer, ImportMemWl, Frame,
};
use smithay::utils::Size;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::backend::renderer::allocator::Fourcc;
use pixman::Image;
use wayland_server::protocol::wl_buffer::WlBuffer;

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
    // Create headless memory buffer in RGBA format
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
    buffer: &Image<'static, 'static>,
) -> Result<(), Box<dyn std::error::Error>> {
    // Get the current buffer attached to the surface
    let wl_buffer = {
        use smithay::wayland::compositor::{with_states, SurfaceAttributes, CachedState};
        with_states(surface, |states| {
            let attrs = states.cached_state.get::<SurfaceAttributes>();
            attrs.current().buffer.as_ref().map(|(_, b)| b.clone())
        })
    };

    let Some(wl_buffer) = wl_buffer else {
        return Err("No buffer attached to surface".into());
    };

    // Import the shared memory buffer
    let texture = renderer.import_shm_buffer(&wl_buffer)?;

    // Get texture dimensions
    let texture_size = texture.size();

    // Bind the offscreen target
    let mut target = renderer.bind(buffer)?;

    // Clear the target
    target.clear([0.0, 0.0, 0.0, 0.0])?;

    // Render the texture to the target
    let frame = renderer.render(&mut target)?;
    frame.render_texture_from_to(
        &texture,
        smithay::utils::Point::default(),
        smithay::utils::Point::default(),
        None,
        1.0,
        smithay::utils::Transform::Normal,
        false,
    )?;

    // Finish the frame
    frame.finish()?;

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
    buffer: &Image<'static, 'static>,
) -> bool {
    render_surface_to_buffer(renderer, surface, buffer).is_ok()
}

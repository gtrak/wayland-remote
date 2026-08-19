//! Offscreen rendering of client surfaces via Smithay's pixman software renderer.
//!
//! [`OffscreenRenderer`] imports committed wl_shm buffers as pixman textures,
//! renders them into a single offscreen BGRA (Argb8888) buffer, and reads the
//! pixels back as a [`FrameBuffer`]. This is the "PRD Step 2" render path: the
//! produced bytes are exactly what the wire carries (issue 05), so GDI can blit
//! them with zero conversion.

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::pixman::{PixmanRenderer, PixmanTexture};
use smithay::backend::renderer::{
    Bind, Color32F, ExportMem, Frame, ImportAll, Offscreen, Renderer, Texture,
};
use smithay::utils::{Buffer as BufferCoord, Physical, Point, Rectangle, Size, Transform};
use std::sync::mpsc::Sender;
use wayland_server::protocol::wl_buffer::WlBuffer;

/// A rendered offscreen frame.
///
/// `data` is a contiguous BGRA (Argb8888) pixel buffer of `width * height * 4`
/// bytes. On little-endian hosts Argb8888 is laid out in memory as
/// `[B, G, R, A]` per pixel, matching the GDI expectation exactly.
#[derive(Debug, Clone)]
pub struct FrameBuffer {
    /// Contiguous BGRA pixel data (`width * height * 4` bytes).
    pub data: Vec<u8>,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Row stride in bytes. For a contiguous Argb8888 buffer this is `width * 4`.
    pub stride: u32,
}

impl FrameBuffer {
    /// Write the frame as an RGBA PNG at `path`.
    ///
    /// The wire format is BGRA; PNG wants RGBA, so R and B channels are swapped
    /// here. This is debug output only — the wire stays BGRA.
    pub fn write_png(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let rgba = self.to_rgba();
        let img = image::RgbaImage::from_raw(self.width, self.height, rgba)
            .ok_or_else(|| anyhow::anyhow!("invalid frame dimensions for PNG: {self:?}"))?;
        img.save(path)?;
        Ok(())
    }

    /// Convert the BGRA data to an RGBA byte vector (R and B swapped).
    #[must_use]
    pub fn to_rgba(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.data.len());
        for pixel in self.data.chunks_exact(4) {
            // BGRA -> RGBA: swap the blue (0) and red (2) channels.
            out.push(pixel[2]);
            out.push(pixel[1]);
            out.push(pixel[0]);
            out.push(pixel[3]);
        }
        out
    }
}

/// A request the compositor thread handles to produce a rendered frame.
pub enum RenderRequest {
    /// Render the current surface set and send the result over `reply`.
    Render {
        /// Reply channel carrying the rendered [`FrameBuffer`] back to the caller.
        reply: Sender<FrameBuffer>,
    },
}

/// Renders committed client surfaces offscreen with the pixman software renderer.
pub struct OffscreenRenderer {
    renderer: PixmanRenderer,
    width: u32,
    height: u32,
}

impl OffscreenRenderer {
    /// Create a renderer targeting a `width` x `height` offscreen framebuffer.
    pub fn new(width: u32, height: u32) -> anyhow::Result<Self> {
        let renderer = PixmanRenderer::new()?;
        Ok(Self {
            renderer,
            width,
            height,
        })
    }

    /// Render `surfaces` (each a `(buffer, x, y)` layout position) into the
    /// offscreen framebuffer and read the pixels back as BGRA.
    pub fn render(&mut self, surfaces: &[(WlBuffer, i32, i32)]) -> anyhow::Result<FrameBuffer> {
        let w = self.width as i32;
        let h = self.height as i32;

        // Import every committed buffer as a texture up front: the `Frame` holds
        // a mutable borrow of the renderer for the whole pass, so imports must
        // happen before the render pass begins.
        let mut textures: Vec<(PixmanTexture, i32, i32)> = Vec::with_capacity(surfaces.len());
        for (buffer, x, y) in surfaces {
            if let Some(Ok(texture)) = self.renderer.import_buffer(buffer, None, &[]) {
                textures.push((texture, *x, *y));
            }
        }

        // Create the offscreen pixel buffer and bind it as the render target.
        let buf_size: Size<i32, BufferCoord> = (w, h).into();
        let mut image = self.renderer.create_buffer(Fourcc::Argb8888, buf_size)?;
        let mut target = self.renderer.bind(&mut image)?;

        // Begin the render pass.
        let out_size: Size<i32, Physical> = (w, h).into();
        let mut frame = self
            .renderer
            .render(&mut target, out_size, Transform::Normal)?;

        // Clear to opaque black. The pixman renderer clips every draw to the
        // supplied damage rects, so the clear rect must cover the whole frame.
        let full: Rectangle<i32, Physical> = Rectangle::new(Point::new(0, 0), Size::new(w, h));
        frame.clear(Color32F::BLACK, &[full])?;

        // Draw each surface at its layout position, over its full extent (the
        // damage rect covers the whole placed texture region).
        for (texture, x, y) in &textures {
            let tsize = texture.size();
            let full_tex: Rectangle<i32, Physical> =
                Rectangle::new(Point::new(0, 0), Size::new(tsize.w, tsize.h));
            frame.render_texture_at(
                texture,
                Point::new(*x, *y),
                1,   // texture_scale
                1.0, // output_scale
                Transform::Normal,
                &[full_tex],
                &[],
                1.0, // alpha
            )?;
        }

        // Finish the pass (frees per-frame resources) and wait for completion.
        // For an Image target this is an already-signaled sync point (a no-op).
        frame
            .finish()?
            .wait()
            .map_err(|_| anyhow::anyhow!("render sync point interrupted"))?;

        // Read the framebuffer back as a contiguous BGRA buffer.
        let region: Rectangle<i32, BufferCoord> = Rectangle::new(Point::new(0, 0), Size::new(w, h));
        let mapping = self
            .renderer
            .copy_framebuffer(&target, region, Fourcc::Argb8888)?;
        let pixels = self.renderer.map_texture(&mapping)?;

        Ok(FrameBuffer {
            data: pixels.to_vec(),
            width: self.width,
            height: self.height,
            stride: self.width * 4,
        })
    }
}

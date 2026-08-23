//! Offscreen rendering of client surfaces via Smithay's pixman software
//! renderer (fallback) or GL (EGL) renderer (imports dmabuf), chosen at
//! startup by [`egl::probe`].
//!
//! [`OffscreenRenderer`] imports committed buffers as textures, renders them
//! into a single offscreen BGRA (Argb8888) buffer, and reads the pixels back
//! as a [`FrameBuffer`]. This is the "PRD Step 2" render path: the produced
//! bytes are exactly what the wire carries (issue 05), so GDI can blit them
//! with zero conversion.

pub mod egl;

use smithay::backend::allocator::Fourcc;
use smithay::backend::renderer::gles::{GlesRenderer, GlesTexture};
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::renderer::{
    Bind, Color32F, ExportMem, Frame, ImportAll, Offscreen as OffscreenTarget, Renderer, Texture,
};
use smithay::utils::{Buffer as BufferCoord, Logical, Physical, Point, Rectangle, Size, Transform};
use smithay::wayland::compositor::{
    BufferAssignment, SubsurfaceCachedState, SurfaceAttributes, get_children, with_states,
};
use std::marker::PhantomData;
use std::sync::mpsc::Sender;
use wayland_server::protocol::wl_buffer::WlBuffer;
use wayland_server::protocol::wl_surface::WlSurface;

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
    /// The window this frame was rendered for; 0 for a full-desktop composite.
    pub window_id: u64,
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
#[derive(Debug)]
pub enum RenderRequest {
    /// Render the current surface set and send the result over `reply`.
    Render {
        /// Reply channel carrying the rendered [`FrameBuffer`] back to the caller.
        reply: Sender<FrameBuffer>,
    },
    /// Render a single mapped window's subsurface tree and send the result
    /// over `reply`.
    RenderWindow {
        /// The window to render.
        window_id: u64,
        /// Reply channel carrying the rendered [`FrameBuffer`] back to the caller.
        reply: Sender<FrameBuffer>,
    },
}

/// Renders committed client surfaces offscreen.
///
/// Generic over the backend renderer `R` and its offscreen target type `T`:
/// both the pixman software renderer (`T = pixman::Image<'static, 'static>`)
/// and the GL (EGL) renderer (`T = GlesTexture`) satisfy the same trait set
/// (`Renderer + ImportAll + Offscreen<T> + Bind<T> + ExportMem`), so the
/// render/readback pipeline is shared verbatim between the two.
pub struct OffscreenRenderer<R, T> {
    renderer: R,
    width: u32,
    height: u32,
    /// Marker tying the renderer to its offscreen target type `T` (the
    /// buffer `create_buffer`/`bind` operate on); never stored.
    _target: PhantomData<T>,
}

impl<R, T> OffscreenRenderer<R, T>
where
    R: Renderer + ImportAll + OffscreenTarget<T> + Bind<T> + ExportMem,
    R::Error: std::error::Error + Send + Sync + 'static,
{
    /// Wrap an already-constructed backend renderer targeting a `width` x
    /// `height` offscreen framebuffer.
    pub fn with_renderer(renderer: R, width: u32, height: u32) -> Self {
        Self {
            renderer,
            width,
            height,
            _target: PhantomData,
        }
    }

    /// Render `surfaces` (each a `(buffer, x, y)` layout position) into the
    /// offscreen framebuffer and read the pixels back as BGRA.
    pub fn render(&mut self, surfaces: &[(WlBuffer, i32, i32)]) -> anyhow::Result<FrameBuffer> {
        let w = self.width as i32;
        let h = self.height as i32;

        // Import every committed buffer as a texture up front: the `Frame` holds
        // a mutable borrow of the renderer for the whole pass, so imports must
        // happen before the render pass begins.
        let mut textures: Vec<(R::TextureId, i32, i32)> = Vec::with_capacity(surfaces.len());
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
            window_id: 0,
        })
    }

    /// Render a single buffer at origin (0,0) into a `width x height` BGRA
    /// target and read it back. `window_id` on the returned frame is 0; the
    /// caller overrides it.
    pub fn render_surface(
        &mut self,
        buffer: &WlBuffer,
        width: u32,
        height: u32,
    ) -> anyhow::Result<FrameBuffer> {
        let w = width as i32;
        let h = height as i32;

        // Import the committed buffer as a texture up front: the `Frame` holds
        // a mutable borrow of the renderer for the whole pass, so the import
        // must happen before the render pass begins.
        let Some(Ok(texture)) = self.renderer.import_buffer(buffer, None, &[]) else {
            anyhow::bail!("failed to import buffer for single-window render");
        };

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

        // Draw the surface at origin, over its full extent (the damage rect
        // covers the whole placed texture region).
        let tsize = texture.size();
        let full_tex: Rectangle<i32, Physical> =
            Rectangle::new(Point::new(0, 0), Size::new(tsize.w, tsize.h));
        frame.render_texture_at(
            &texture,
            Point::new(0, 0),
            1,   // texture_scale
            1.0, // output_scale
            Transform::Normal,
            &[full_tex],
            &[],
            1.0, // alpha
        )?;

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
            width,
            height,
            stride: width * 4,
            window_id: 0,
        })
    }

    /// Render a window's full subsurface tree (root + all (sub)surfaces) into
    /// a `width x height` BGRA target and read it back. `window_id` on the
    /// returned frame is 0; the caller overrides it.
    ///
    /// Each (sub)surface is drawn at its position accumulated relative to the
    /// window origin; the pixman renderer clips every draw to the target, so
    /// subsurfaces sticking out of the window rect are simply cut off.
    ///
    /// TODO: apply `ViewportCachedState` (viewporter) source-crop /
    /// destination-scale per surface before importing — MVP renders each
    /// buffer at its natural size and position.
    pub fn render_window_surface(
        &mut self,
        root: &WlSurface,
        width: u32,
        height: u32,
        cursor: Option<(&WlSurface, Point<i32, Logical>)>,
    ) -> anyhow::Result<FrameBuffer> {
        let w = width as i32;
        let h = height as i32;

        // Walk the subsurface tree, accumulating each surface's offset
        // relative to the window origin.
        let mut placed: Vec<(WlSurface, Point<i32, Logical>)> = Vec::new();
        collect_surfaces(root, Point::new(0, 0), &mut placed);

        // Import every committed buffer as a texture up front: the `Frame`
        // holds a mutable borrow of the renderer for the whole pass, so the
        // imports must happen before the render pass begins.
        let mut textures: Vec<(R::TextureId, i32, i32)> = Vec::with_capacity(placed.len());
        for (surface, offset) in &placed {
            if let Some(texture) = self.committed_texture(surface) {
                textures.push((texture, offset.x, offset.y));
            }
        }

        // Import the cursor texture up front too (same borrow constraint as the
        // surface textures above): the `Frame` holds `&mut renderer` for the
        // whole pass, so its import must happen before the pass begins.
        let cursor_tex: Option<(R::TextureId, Point<i32, Logical>)> = cursor
            .and_then(|(cursor_surface, pos)| {
                self.committed_texture(cursor_surface).map(|tex| (tex, pos))
            });

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

        // Draw each (sub)surface at its accumulated position, over its full
        // extent (the damage rect covers the whole placed texture region).
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

        // Draw the pointer cursor on top of the window content, if present.
        if let Some((tex, pos)) = &cursor_tex {
            let csize = tex.size();
            let full_cursor: Rectangle<i32, Physical> =
                Rectangle::new(Point::new(0, 0), Size::new(csize.w, csize.h));
            frame.render_texture_at(
                tex,
                Point::new(pos.x, pos.y),
                1,   // texture_scale
                1.0, // output_scale
                Transform::Normal,
                &[full_cursor],
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
            width,
            height,
            stride: width * 4,
            window_id: 0,
        })
    }

    /// Import a surface's committed buffer as a texture, if it currently has
    /// one attached. Surfaces without a committed buffer (or with an
    /// unimportable one) are skipped and simply not drawn.
    fn committed_texture(&mut self, surface: &WlSurface) -> Option<R::TextureId> {
        let buffer = with_states(surface, |states| {
            states
                .cached_state
                .get::<SurfaceAttributes>()
                .current()
                .buffer
                .as_ref()
                .and_then(|assignment| match assignment {
                    BufferAssignment::NewBuffer(buffer) => Some(buffer.clone()),
                    BufferAssignment::Removed => None,
                })
        })?;
        let Some(Ok(texture)) = self.renderer.import_buffer(&buffer, None, &[]) else {
            return None;
        };
        Some(texture)
    }
}

impl OffscreenRenderer<PixmanRenderer, smithay::reexports::pixman::Image<'static, 'static>> {
    /// Create a pixman software renderer targeting a `width` x `height`
    /// offscreen framebuffer.
    pub fn new(width: u32, height: u32) -> anyhow::Result<Self> {
        let renderer = PixmanRenderer::new()?;
        Ok(Self::with_renderer(renderer, width, height))
    }
}

/// The active offscreen renderer, chosen at startup: the GL (EGL) renderer
/// when [`egl::probe`] finds a usable DRM render node (can import dmabuf
/// buffers), or the pixman software renderer (wl_shm only) as the fallback.
pub enum Offscreen {
    /// The pixman software renderer (fallback; wl_shm buffers only).
    Software(
        OffscreenRenderer<PixmanRenderer, smithay::reexports::pixman::Image<'static, 'static>>,
    ),
    /// The GL (EGL) renderer (can import dmabuf buffers). Boxed: the GL
    /// renderer is large, and boxing keeps the software variant (the common
    /// headless case) cheap to store in [`crate::state::State`].
    Gl(Box<OffscreenRenderer<GlesRenderer, GlesTexture>>),
}

impl Offscreen {
    /// Render `surfaces` (each a `(buffer, x, y)` layout position) into the
    /// offscreen framebuffer and read the pixels back as BGRA.
    pub fn render(&mut self, surfaces: &[(WlBuffer, i32, i32)]) -> anyhow::Result<FrameBuffer> {
        match self {
            Self::Software(r) => r.render(surfaces),
            Self::Gl(r) => r.render(surfaces),
        }
    }

    /// Render a single buffer at origin (0,0) into a `width x height` BGRA
    /// target and read it back.
    pub fn render_surface(
        &mut self,
        buffer: &WlBuffer,
        width: u32,
        height: u32,
    ) -> anyhow::Result<FrameBuffer> {
        match self {
            Self::Software(r) => r.render_surface(buffer, width, height),
            Self::Gl(r) => r.render_surface(buffer, width, height),
        }
    }

    /// Render a window's full subsurface tree (root + all (sub)surfaces) into
    /// a `width x height` BGRA target and read it back.
    pub fn render_window_surface(
        &mut self,
        root: &WlSurface,
        width: u32,
        height: u32,
        cursor: Option<(&WlSurface, Point<i32, Logical>)>,
    ) -> anyhow::Result<FrameBuffer> {
        match self {
            Self::Software(r) => r.render_window_surface(root, width, height, cursor),
            Self::Gl(r) => r.render_window_surface(root, width, height, cursor),
        }
    }
}

/// Collect `surface` and all of its (recursive) subsurfaces, each paired with
/// its position accumulated relative to the walk's origin (`offset` for
/// `surface` itself, parent offsets plus the child's subsurface location for
/// descendants). Surfaces are emitted parent-before-children, so children are
/// drawn on top of their parents.
fn collect_surfaces(
    surface: &WlSurface,
    offset: Point<i32, Logical>,
    out: &mut Vec<(WlSurface, Point<i32, Logical>)>,
) {
    out.push((surface.clone(), offset));
    for child in get_children(surface) {
        let child_offset = with_states(&child, |states| {
            let loc = states
                .cached_state
                .get::<SubsurfaceCachedState>()
                .current()
                .location;
            Point::new(offset.x + loc.x, offset.y + loc.y)
        });
        collect_surfaces(&child, child_offset, out);
    }
}

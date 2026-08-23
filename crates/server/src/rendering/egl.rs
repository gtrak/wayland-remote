//! EGL/GBM render-node probe (issue 09, subtask 09a).
//!
//! Probes for a usable DRM render node (`/dev/dri/renderD*`) and, on the first
//! one where the full EGL→GL chain succeeds, builds a Smithay [`GlesRenderer`]
//! capable of importing dmabuf buffers. On a box with a working GPU driver this
//! returns [`Some`]; on a GPU-less box (or a vendor node whose software EGL path
//! fails to initialise) it returns [`None`] so the caller falls back to the
//! pixman software renderer.
//!
//! This module is the probe only (09a): it is not yet wired into
//! [`crate::run`] or the [`crate::rendering::OffscreenRenderer`]. The
//! environment evidence and the verified constructor chain live in the
//! `egl-dmabuf-feasibility` skill.

// The EGL entry points (`EGLDisplay::new`, `GlesRenderer::new`) are inherently
// `unsafe`: the caller must guarantee the context is not current on another
// thread. The workspace denies `unsafe_code`; allow it here — mirroring the
// Win32 display module — and confine it to the two `unsafe` blocks in
// [`probe_node`].
#![allow(unsafe_code)]

use std::path::{Path, PathBuf};

use smithay::backend::allocator::gbm::GbmDevice;
use smithay::backend::allocator::Format;
use smithay::backend::drm::DrmNode;
use smithay::backend::egl::{EGLContext, EGLDisplay};
use smithay::backend::renderer::gles::GlesRenderer;

/// A ready-to-use GPU (EGL/GL) rendering setup, probed from a DRM render node.
///
/// `renderer` is `!Send` (it owns an EGL context) and must stay on the
/// compositor thread. `main_device` is the chosen render node's `dev_t`, used
/// later to build the `zwp_linux_dmabuf` feedback global; `formats` is the
/// display's dmabuf render format set.
pub struct GlesSetup {
    /// The GL renderer built over the probed EGL display.
    pub renderer: GlesRenderer,
    /// The DRM `dev_t` of the chosen render node.
    pub main_device: libc::dev_t,
    /// The display's dmabuf render formats.
    pub formats: Vec<Format>,
}

/// Probe for a usable EGL render node and build a [`GlesSetup`].
///
/// Globs `/dev/dri/renderD*` (in sorted order for a deterministic probe) and
/// returns the first node where the full chain
/// `File → GbmDevice → EGLDisplay → EGLContext → GlesRenderer` succeeds. Any
/// per-node failure is logged with the node path and the next node is tried.
/// If no node works, a warning is logged and [`None`] is returned.
#[must_use]
pub fn probe() -> Option<GlesSetup> {
    let entries = match std::fs::read_dir("/dev/dri") {
        Ok(entries) => entries,
        Err(err) => {
            tracing::warn!(?err, "/dev/dri not readable; falling back to software (pixman)");
            return None;
        }
    };

    // Collect the render nodes, sorted so the probe order is deterministic
    // (e.g. renderD128 before renderD129).
    let mut nodes: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("renderD"))
        })
        .collect();
    nodes.sort();

    if nodes.is_empty() {
        tracing::warn!("no /dev/dri/renderD* nodes found; falling back to software (pixman)");
        return None;
    }

    for node in &nodes {
        match probe_node(node) {
            Ok(setup) => {
                tracing::info!(
                    node = %node.display(),
                    format_count = setup.formats.len(),
                    "EGL render node selected"
                );
                return Some(setup);
            }
            Err(err) => {
                tracing::warn!(node = %node.display(), %err, "EGL probe failed for render node");
            }
        }
    }

    tracing::warn!("no usable EGL render node; falling back to software (pixman)");
    None
}

/// Build a [`GlesSetup`] from a single render node, or an error naming the
/// first step that failed.
fn probe_node(path: &Path) -> Result<GlesSetup, Box<dyn std::error::Error>> {
    let file = std::fs::File::open(path)?;
    let gbm_device = GbmDevice::new(file)?;
    // SAFETY: we construct the display on the compositor thread and the EGL
    // context is not current on any other thread.
    let display = unsafe { EGLDisplay::new(gbm_device)? };

    // Copy the formats out and stat the node before `EGLContext::new` borrows
    // the display: the formats are owned data we keep, and the `dev_t` comes
    // from a fresh stat of the node path.
    let formats: Vec<Format> = display.dmabuf_render_formats().iter().copied().collect();
    let main_device = DrmNode::from_path(path)?.dev_id();

    let context = EGLContext::new(&display)?;
    // SAFETY: the context is fresh and not current on any other thread.
    let renderer = unsafe { GlesRenderer::new(context)? };

    Ok(GlesSetup {
        renderer,
        main_device,
        formats,
    })
}

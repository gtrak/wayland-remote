//! Shared state and protocol handler wiring for the headless compositor.
//!
//! [`State`] owns the Wayland protocol state (compositor, shm, seat, output),
//! tracks committed surfaces by object id, and reports surface-count changes
//! over an optional status channel. The `delegate_*!` macros forward
//! protocol dispatch to the per-global state fields.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{Receiver, Sender};
use std::time::{Duration, Instant};

use smithay::backend::allocator::dmabuf::Dmabuf;
use smithay::backend::allocator::Buffer as _;
use smithay::backend::allocator::Format;
use smithay::delegate_compositor;
use smithay::delegate_data_device;
use smithay::delegate_dmabuf;
use smithay::delegate_output;
use smithay::delegate_seat;
use smithay::delegate_shm;
use smithay::delegate_text_input_manager;
use smithay::delegate_viewporter;
use smithay::delegate_xdg_shell;
use smithay::input::keyboard::XkbConfig;
use smithay::input::pointer::{CursorImageStatus, CursorImageSurfaceData};
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::utils::{Point, Serial, Size, Transform};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    BufferAssignment, CompositorClientState, CompositorHandler, CompositorState, SurfaceAttributes,
    get_role, with_states,
};
use smithay::wayland::dmabuf::{
    DmabufFeedbackBuilder, DmabufGlobal, DmabufHandler, DmabufState, ImportNotifier, get_dmabuf,
};
use smithay::wayland::output::{OutputHandler, OutputManagerState};
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::selection::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
};
use smithay::wayland::shell::xdg::{
    Configure, PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
    XdgToplevelSurfaceData,
};
use smithay::wayland::shm::{ShmHandler, ShmState, with_buffer_contents};
use smithay::wayland::text_input::TextInputManagerState;
use smithay::wayland::viewporter::ViewporterState;
use wayland_remote_protocol::{Compression, InputEvent};
use wayland_server::backend::{ClientData, ObjectId};
use wayland_server::protocol::wl_buffer::WlBuffer;
use wayland_server::protocol::wl_seat;
use wayland_server::protocol::wl_shell;
use wayland_server::protocol::wl_shell_surface;
use wayland_server::protocol::wl_surface::WlSurface;
use wayland_server::{Client, DisplayHandle, Resource, delegate_dispatch, delegate_global_dispatch};

use crate::bridge::NetCommand;
use crate::rendering::{FrameBuffer, Offscreen, RenderRequest};
use crate::wl_shell::WlShellState;

/// Configuration for the headless compositor.
#[derive(Clone, Debug)]
pub struct Config {
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// Socket name to bind (created under `$XDG_RUNTIME_DIR`); auto-named if `None`.
    pub socket_name: Option<String>,
    /// If set, render once after the first client commits a surface, write the
    /// frame as a PNG to this path, and exit.
    pub snapshot: Option<PathBuf>,
    /// QUIC listen address; `None` disables networking entirely.
    pub listen: Option<SocketAddr>,
    /// Frame payload compression used by the QUIC frame server.
    pub compression: Compression,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            socket_name: None,
            snapshot: None,
            listen: None,
            compression: Compression::Lz4,
        }
    }
}

/// Per-client state stored as `ClientData`, holding the compositor's
/// per-client state so it is cleaned up automatically on disconnect.
#[derive(Debug)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {}

impl std::ops::Deref for ClientState {
    type Target = CompositorClientState;
    fn deref(&self) -> &Self::Target {
        &self.compositor_state
    }
}

/// Per-surface render information: the committed buffer and its layout
/// position in the trivial tiling (0,0 / 20,20 / 40,40 … for M1).
#[derive(Debug, Clone)]
pub struct SurfaceInfo {
    /// The committed buffer, if the surface currently has one attached.
    pub buffer: Option<WlBuffer>,
    /// Layout x position in the offscreen frame.
    pub x: i32,
    /// Layout y position in the offscreen frame.
    pub y: i32,
    /// Committed buffer width in pixels.
    pub width: u32,
    /// Committed buffer height in pixels.
    pub height: u32,
}

/// Server-side telemetry counters for observability and the test harness.
pub struct Telemetry {
    frames_total: u64,
    frames_this_second: u64,
    frame_bytes_total: u64,
    render_ns_this_second: u64,
    readback_ns_this_second: u64,
    commits_total: u64,
    input_events_total: u64,
    last_input_at: Option<Instant>,
    last_input_to_commit_ms: Option<u32>,
    errors_total: u64,
    second_start: Instant,
}

/// A cheap-to-copy snapshot of telemetry for logging.
#[derive(Debug, Clone)]
pub struct TelemetrySnapshot {
    pub frames_per_sec: u64,
    pub frames_total: u64,
    pub frame_bytes_total: u64,
    pub render_ms: u64,
    pub readback_ms: u64,
    pub commits_total: u64,
    pub input_events_total: u64,
    pub last_input_to_commit_ms: Option<u32>,
    pub errors_total: u64,
}

impl Telemetry {
    /// Create a telemetry counter set with all counters zeroed and the
    /// per-second window starting now.
    #[must_use]
    pub fn new() -> Self {
        Self {
            frames_total: 0,
            frames_this_second: 0,
            frame_bytes_total: 0,
            render_ns_this_second: 0,
            readback_ns_this_second: 0,
            commits_total: 0,
            input_events_total: 0,
            last_input_at: None,
            last_input_to_commit_ms: None,
            errors_total: 0,
            second_start: Instant::now(),
        }
    }

    /// Record a surface commit. If an input event arrived within the last 5
    /// seconds, capture the input-to-commit latency; the pending input
    /// timestamp is always cleared so each latency sample pairs one input
    /// with the next commit.
    pub fn record_commit(&mut self) {
        self.commits_total += 1;
        if let Some(t) = self.last_input_at {
            let elapsed = t.elapsed();
            if elapsed < Duration::from_secs(5) {
                self.last_input_to_commit_ms = Some(elapsed.as_millis() as u32);
            }
        }
        self.last_input_at = None;
    }

    /// Record a network input event and stamp the time so the next commit
    /// can measure input-to-commit latency.
    pub fn record_input(&mut self) {
        self.input_events_total += 1;
        self.last_input_at = Some(Instant::now());
    }

    /// Record a streamed frame of `bytes` payload size, with `render_ns` and
    /// `readback_ns` render/readback timings (nanoseconds, 0 when not measured).
    pub fn record_frame(&mut self, bytes: usize, render_ns: u64, readback_ns: u64) {
        self.frames_total += 1;
        self.frames_this_second += 1;
        self.frame_bytes_total += bytes as u64;
        self.render_ns_this_second += render_ns;
        self.readback_ns_this_second += readback_ns;
    }

    /// Record a recoverable error (failed render/snapshot).
    pub fn record_error(&mut self) {
        self.errors_total += 1;
    }

    /// Elapsed time since the per-second window started, for cheap polling
    /// without mutating the counters.
    #[must_use]
    pub fn second_start_elapsed(&self) -> Duration {
        self.second_start.elapsed()
    }

    /// Snapshot the cumulative counters. If at least one second has passed
    /// since the last snapshot, also publish and reset the per-second frame
    /// rate; otherwise `frames_per_sec` is 0 and the window keeps counting.
    pub fn snapshot(&mut self) -> TelemetrySnapshot {
        let (frames_per_sec, render_ms, readback_ms) =
            if self.second_start.elapsed() >= Duration::from_secs(1) {
                let fps = self.frames_this_second;
                let render_ms = self.render_ns_this_second / 1_000_000;
                let readback_ms = self.readback_ns_this_second / 1_000_000;
                self.frames_this_second = 0;
                self.render_ns_this_second = 0;
                self.readback_ns_this_second = 0;
                self.second_start = Instant::now();
                (fps, render_ms, readback_ms)
            } else {
                (0, 0, 0)
            };
        TelemetrySnapshot {
            frames_per_sec,
            frames_total: self.frames_total,
            frame_bytes_total: self.frame_bytes_total,
            render_ms,
            readback_ms,
            commits_total: self.commits_total,
            input_events_total: self.input_events_total,
            last_input_to_commit_ms: self.last_input_to_commit_ms,
            errors_total: self.errors_total,
        }
    }
}

impl Default for Telemetry {
    fn default() -> Self {
        Self::new()
    }
}

/// All compositor state: globals, tracked surfaces, and shutdown plumbing.
pub struct State {
    pub display_handle: DisplayHandle,
    pub compositor_state: CompositorState,
    pub shm_state: ShmState,
    pub xdg_shell_state: XdgShellState,
    pub seat_state: SeatState<State>,
    pub seat: Seat<State>,
    pub output: Output,
    pub output_manager_state: OutputManagerState,
    /// Handles wp_viewporter (surface cropping/scaling) requests.
    pub viewporter_state: ViewporterState,
    /// Handles wl_data_device_manager (clipboard / DnD) requests.
    pub data_device_state: DataDeviceState,
    /// Handles legacy wl_shell requests (older clients map toplevels via
    /// this deprecated global).
    pub wl_shell_state: WlShellState,
    /// Handles zwp_text_input_v3 requests (IME-aware clients bind this). No IME
    /// engine is present — the global's presence stops "No text input manager"
    /// warnings; typing works via the keyboard path.
    pub text_input_manager_state: TextInputManagerState,
    /// Handles zwp_linux_dmabuf requests (EGL/dmabuf clients); the global is
    /// registered only when an EGL render node was probed at startup.
    pub dmabuf_state: DmabufState,
    /// The surface set via `wl_pointer.set_cursor`, if any. Drawn on top of the
    /// focused window at render time.
    pub cursor_surface: Option<WlSurface>,
    /// Identity of the cursor buffer last read back + sent as `CursorShape`.
    /// A `cursor_image` call whose committed buffer matches this skips the
    /// (synchronous) GPU readback, so a static cursor is read back once and
    /// pure pointer moves are no-ops. Cleared on `Hidden`/`Named`.
    cursor_sprite_buffer: Option<WlBuffer>,
    /// Tracks xdg toplevels, window ids, focus, and pending window events.
    pub window_manager: crate::window::WindowManager,
    /// Committed surfaces, keyed by object id, with buffer + layout position.
    pub surfaces: HashMap<ObjectId, SurfaceInfo>,
    /// Offscreen renderer (pixman fallback or GL/dmabuf), initialized after
    /// display setup.
    pub renderer: Option<Offscreen>,
    /// Test back-channel carrying render requests (None in production).
    pub render_rx: Option<Receiver<RenderRequest>>,
    pub config: Config,
    /// Test back-channel reporting the current surface count.
    pub status_tx: Option<Sender<usize>>,
    /// Set by the signal source (or externally) to request shutdown.
    pub shutdown: Arc<AtomicBool>,
    /// Input event router (injects network input into the smithay seat).
    pub input_router: crate::input::InputRouter,
    /// Tracks whether a `--snapshot` frame has been written (exactly once).
    pub snapshot_done: bool,
    /// Server-side telemetry counters for observability and the test harness.
    pub telemetry: Telemetry,
    /// Optional sender to the QUIC net side for cursor updates; `None` when
    /// networking is off / in unit tests.
    pub net_cmd_tx: Option<tokio::sync::mpsc::UnboundedSender<NetCommand>>,
    /// Server start, used as the monotonic time base for frame callbacks.
    start: Instant,
}

impl State {
    /// Build the full state: compositor, shm, seat (keyboard + pointer), and output.
    pub fn new(
        display_handle: DisplayHandle,
        config: Config,
        status_tx: Option<Sender<usize>>,
        render_rx: Option<Receiver<RenderRequest>>,
        shutdown: Arc<AtomicBool>,
        dmabuf_global: Option<(libc::dev_t, Vec<Format>)>,
    ) -> anyhow::Result<Self> {
        let compositor_state = CompositorState::new::<State>(&display_handle);
        let shm_state = ShmState::new::<State>(&display_handle, vec![]);
        let xdg_shell_state = XdgShellState::new::<State>(&display_handle);
        let viewporter_state = ViewporterState::new::<State>(&display_handle);
        let data_device_state = DataDeviceState::new::<State>(&display_handle);
        let text_input_manager_state = TextInputManagerState::new::<State>(&display_handle);
        let wl_shell_state = WlShellState::new(&display_handle);

        // Register the zwp_linux_dmabuf global only when the EGL probe found a
        // render node: the feedback advertises that node's dev_t + formats so
        // EGL/dmabuf clients (e.g. weston-simple-egl) can attach buffers.
        let mut dmabuf_state = DmabufState::new();
        if let Some((main_device, formats)) = dmabuf_global {
            let feedback = DmabufFeedbackBuilder::new(main_device, formats).build()?;
            let _global = dmabuf_state
                .create_global_with_default_feedback::<State>(&display_handle, &feedback);
            tracing::info!("zwp_linux_dmabuf global registered (EGL/dmabuf clients enabled)");
        }

        let mut seat_state = SeatState::<State>::new();
        let mut seat = seat_state.new_wl_seat(&display_handle, "wayland-remote");
        seat.add_keyboard(XkbConfig::default(), 25, 600)?;
        seat.add_pointer();

        let output = Output::new(
            "wayland-remote".to_owned(),
            PhysicalProperties {
                size: Size::new(1280, 720),
                subpixel: Subpixel::Unknown,
                make: "wayland-remote".to_owned(),
                model: "headless".to_owned(),
            },
        );
        let mode = Mode {
            size: Size::new(config.width as i32, config.height as i32),
            refresh: 60_000,
        };
        output.set_preferred(mode);
        output.change_current_state(
            Some(mode),
            Some(Transform::Normal),
            Some(Scale::Integer(1)),
            Some(Point::new(0, 0)),
        );
        let output_manager_state = OutputManagerState::new();
        output.create_global::<State>(&display_handle);

        Ok(Self {
            display_handle,
            compositor_state,
            shm_state,
            xdg_shell_state,
            seat_state,
            seat,
            output,
            output_manager_state,
            viewporter_state,
            data_device_state,
            wl_shell_state,
            text_input_manager_state,
            dmabuf_state,
            cursor_surface: None,
            cursor_sprite_buffer: None,
            window_manager: crate::window::WindowManager::new(),
            surfaces: HashMap::new(),
            renderer: None,
            render_rx,
            config,
            status_tx,
            shutdown,
            input_router: crate::input::InputRouter::new(),
            snapshot_done: false,
            telemetry: Telemetry::new(),
            net_cmd_tx: None,
            start: Instant::now(),
        })
    }

    /// Number of tracked committed surfaces.
    #[must_use]
    pub fn surface_count(&self) -> usize {
        self.surfaces.len()
    }

    /// Inject a network input event into the smithay seat.
    pub fn inject_input(&mut self, window_id: u64, event: InputEvent, serial: Serial, time: u32) {
        self.telemetry.record_input();
        crate::input::inject(self, window_id, event, serial, time);
    }

    /// Render the currently committed surfaces offscreen and read the pixels
    /// back as a BGRA [`FrameBuffer`].
    pub fn render_frame(&mut self) -> anyhow::Result<FrameBuffer> {
        let surfaces: Vec<(WlBuffer, i32, i32)> = self
            .surfaces
            .values()
            .filter_map(|info| {
                info.buffer
                    .as_ref()
                    .map(|buffer| (buffer.clone(), info.x, info.y))
            })
            .collect();
        let renderer = self
            .renderer
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no offscreen renderer configured"))?;
        renderer.render(&surfaces)
    }

    /// Render a single mapped window's subsurface tree at its current size.
    pub fn render_window(&mut self, window_id: u64) -> anyhow::Result<FrameBuffer> {
        let (width, height) = self
            .window_manager
            .window_size(window_id)
            .ok_or_else(|| anyhow::anyhow!("window {window_id} not mapped"))?;
        let surface = self
            .window_manager
            .surface_for(window_id)
            .ok_or_else(|| anyhow::anyhow!("window {window_id} not found"))?
            .clone();

        // The pointer cursor is no longer composited into the frame: it is a
        // native viewer-side cursor (issue 04c), so render the window content
        // without it.
        let renderer = self
            .renderer
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no offscreen renderer configured"))?;
        let mut frame = renderer.render_window_surface(&surface, width, height, None)?;
        frame.window_id = window_id;
        Ok(frame)
    }

    /// Resolve the window id a cursor update should target: the pointer-focus
    /// window when the pointer is over a tracked toplevel, else the focused
    /// window. `None` when neither yields a tracked window — in that case the
    /// cursor update has no target to emit to and is skipped.
    fn cursor_window_id(&self) -> Option<u64> {
        self.seat
            .get_pointer()
            .and_then(|p| p.current_focus())
            .and_then(|surface| self.window_manager.window_for_surface(&surface))
            .or_else(|| self.window_manager.focused())
    }

    /// The cursor surface's currently-committed buffer, or `None` if it has
    /// none. The client attaches + commits it before `set_cursor`, so it is in
    /// `current()` by the time `cursor_image` runs. This committed `WlBuffer`
    /// (compared by `ObjectId`, which is unique per client connection) is the
    /// cursor-sprite cache key: it only changes when a client sets a new image.
    fn cursor_committed_buffer(&self, cursor: &WlSurface) -> Option<WlBuffer> {
        with_states(cursor, |states| {
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
        })
    }

    /// Read back a cursor sprite buffer's pixels (BGRA) via the offscreen
    /// renderer, returning the frame on success. Any failure (no renderer, zero
    /// size, unimportable buffer) yields `None` — a cursor readback must never
    /// panic or drop the client connection.
    fn readback_cursor_sprite(&mut self, buffer: &WlBuffer) -> Option<FrameBuffer> {
        // SHM buffers expose their size via `with_buffer_contents`; dmabuf
        // buffers carry a `Dmabuf` in the data map instead (same fallback as
        // `commit`).
        let (width, height) =
            with_buffer_contents(buffer, |_, _, data| (data.width, data.height))
                .ok()
                .or_else(|| {
                    get_dmabuf(buffer)
                        .ok()
                        .map(|d| (d.width() as i32, d.height() as i32))
                })?;
        let width = u32::try_from(width).unwrap_or(0);
        let height = u32::try_from(height).unwrap_or(0);
        if width == 0 || height == 0 {
            tracing::debug!(
                "cursor sprite has no committed buffer or a zero size; skipping readback"
            );
            return None;
        }
        // Render the sprite buffer into a `width x height` target at origin and
        // read the pixels back (the existing single-surface readback path).
        self.renderer
            .as_mut()
            .and_then(|renderer| renderer.render_surface(buffer, width, height).ok())
    }

    fn report_surface_count(&self) {
        if let Some(tx) = &self.status_tx {
            let _ = tx.send(self.surfaces.len());
        }
    }
}

impl CompositorHandler for State {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        client
            .get_data::<ClientState>()
            .map(|cs| &cs.compositor_state)
            .expect("every client is inserted with a ClientState")
    }

    fn commit(&mut self, surface: &WlSurface) {
        // Fire present-completion frame callbacks for this commit so
        // wl_surface.frame-paced clients (e.g. weston-simple-egl) advance.
        // smithay merged the pending callbacks into current() just before this
        // handler ran; draining moves each one out so it fires exactly once
        // (the next commit repopulates the vec).
        let time = self.start.elapsed().as_millis() as u32;
        with_states(surface, |states| {
            let mut guard = states.cached_state.get::<SurfaceAttributes>();
            for callback in guard.current().frame_callbacks.drain(..) {
                callback.done(time);
            }
        });

        // The cursor surface is drawn explicitly on top of the focused window at
        // render time; skip tiling it into the surface map.
        if get_role(surface) == Some("cursor_image") {
            return;
        }
        let committed = with_states(surface, |states| {
            let mut guard = states.cached_state.get::<SurfaceAttributes>();
            let attrs = guard.current();
            match attrs.buffer {
                Some(BufferAssignment::NewBuffer(ref buffer)) => {
                    // SHM buffers expose their size via with_buffer_contents; dmabuf
                    // buffers carry a `Dmabuf` in their data map instead (with_buffer_contents
                    // returns NotManaged for them), so fall back to get_dmabuf.
                    let (width, height) =
                        with_buffer_contents(buffer, |_, _, data| (data.width, data.height))
                            .ok()
                            .or_else(|| {
                                get_dmabuf(buffer)
                                    .ok()
                                    .map(|d| (d.width() as i32, d.height() as i32))
                            })
                            .unwrap_or((0, 0));
                    let width = u32::try_from(width).unwrap_or(0);
                    let height = u32::try_from(height).unwrap_or(0);
                    Some((buffer.clone(), width, height))
                }
                _ => None,
            }
        });
        // A `Some` commit attached a new buffer (pixels may have changed).
        // Captured before `committed` is moved into the `info` match below.
        let new_buffer = committed.is_some();
        self.telemetry.record_commit();

        let id = surface.id();
        let existing = self.surfaces.get(&id).cloned();
        // Trivial tiling: first committed surface at (0,0), each subsequent one
        // offset diagonally by 20px. Re-commits (e.g. resize) keep their slot.
        let (x, y) = match &existing {
            Some(info) => (info.x, info.y),
            None => {
                let index = self.surfaces.len() as i32;
                (index * 20, index * 20)
            }
        };
        let info = match (committed, existing) {
            (Some((buffer, width, height)), _prev) => SurfaceInfo {
                buffer: Some(buffer),
                width,
                height,
                x,
                y,
            },
            (None, prev) => match prev {
                Some(mut prev) => {
                    prev.buffer = None;
                    prev
                }
                None => SurfaceInfo {
                    buffer: None,
                    width: 0,
                    height: 0,
                    x,
                    y,
                },
            },
        };
        // xdg toplevel mapping: the first commit after the client acked its
        // initial configure maps the window and queues a Created event.
        let (width, height) = (info.width, info.height);
        self.surfaces.insert(id, info);
        self.report_surface_count();
        self.window_manager.on_commit(surface, width, height);
        // A newly committed buffer may have changed the window's pixels: mark
        // every mapped window dirty so the stream loop re-renders it.
        if new_buffer {
            self.window_manager.mark_all_mapped_dirty();
        }

        // Toplevels store the client-set title in their role data (set_title
        // is not double-buffered); sync it so window events carry it.
        let title = with_states(surface, |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .and_then(|data| data.lock().ok().and_then(|guard| guard.title.clone()))
        });
        if let Some(title) = title {
            self.window_manager.set_title(surface, title);
        }

        tracing::debug!(x, y, "surface commit");
    }

    fn destroyed(&mut self, surface: &WlSurface) {
        if self.surfaces.remove(&surface.id()).is_some() {
            self.report_surface_count();
        }
        // Clear the cursor surface if the destroyed surface was the cursor.
        if self
            .cursor_surface
            .as_ref()
            .map(|s| s.id())
            .is_some_and(|id| id == surface.id())
        {
            self.cursor_surface = None;
        }
        // A tracked window dies with its surface. For xdg toplevels this is
        // usually a no-op (toplevel_destroyed already removed it); for
        // legacy wl_shell windows this is the destruction path (the shell
        // surface object has no independent lifetime).
        self.window_manager.destroy(surface);
    }
}

impl ShmHandler for State {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl BufferHandler for State {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {
        // Buffers are reference-counted by the protocol; no per-buffer
        // state is kept yet.
    }
}

impl DmabufHandler for State {
    fn dmabuf_state(&mut self) -> &mut DmabufState {
        &mut self.dmabuf_state
    }

    fn dmabuf_imported(&mut self, _global: &DmabufGlobal, _dmabuf: Dmabuf, notifier: ImportNotifier) {
        // Acknowledge the import so the client's wl_buffer is created. The
        // actual texture import happens lazily at render time (OffscreenRenderer
        // -> import_buffer -> import_dmabuf on the GL renderer).
        if let Err(err) = notifier.successful::<State>() {
            tracing::warn!(?err, "dmabuf import: could not acknowledge (client may have died)");
        }
    }
}

impl SeatHandler for State {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<State> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, _seat: &Seat<State>, _focused: Option<&WlSurface>) {}

    fn cursor_image(&mut self, _seat: &Seat<State>, image: CursorImageStatus) {
        match image {
            // A drawable cursor: keep it for (legacy) in-frame use, then relay
            // its sprite to viewers. Any failure — no target window, no
            // renderer, no committed buffer, unimportable buffer — is a no-op;
            // a cursor update must never panic or drop the client.
            CursorImageStatus::Surface(s) => {
                self.cursor_surface = Some(s.clone());
                // Cache the cursor sprite: only re-read + re-emit when the
                // cursor surface's committed buffer changes (a client set a new
                // cursor image). The committed `WlBuffer` is compared by
                // `ObjectId`, so a static cursor is read back once and the
                // (synchronous) GPU readback is skipped on the `cursor_image`
                // calls fired by pure pointer moves.
                let buffer = self.cursor_committed_buffer(&s);
                if buffer.as_ref() == self.cursor_sprite_buffer.as_ref() {
                    return;
                }
                if let Some(window_id) = self.cursor_window_id()
                    && let Some(buffer) = buffer.as_ref()
                    && let Some(frame) = self.readback_cursor_sprite(buffer)
                    && let Some(tx) = &self.net_cmd_tx
                {
                    // The hotspot is on the cursor surface's data map (written by
                    // `set_cursor` before this handler fired).
                    let hotspot = with_states(&s, |states| {
                        states
                            .data_map
                            .get::<CursorImageSurfaceData>()
                            .map(|d| d.lock().unwrap().hotspot)
                            .unwrap_or_default()
                    });
                    let _ = tx.send(NetCommand::CursorShape {
                        window_id,
                        width: frame.width,
                        height: frame.height,
                        hot_x: hotspot.x,
                        hot_y: hotspot.y,
                        data: frame.data,
                    });
                    self.cursor_sprite_buffer = Some(buffer.clone());
                }
            }
            // No cursor theme in the headless MVP: `Hidden`/`Named` clear the
            // in-frame cursor, reset the sprite cache so a later `Surface`
            // cursor re-reads, and tell viewers to hide theirs.
            CursorImageStatus::Hidden | CursorImageStatus::Named(_) => {
                self.cursor_surface = None;
                self.cursor_sprite_buffer = None;
                if let Some(window_id) = self.cursor_window_id()
                    && let Some(tx) = &self.net_cmd_tx
                {
                    let _ = tx.send(NetCommand::CursorHide { window_id });
                }
            }
        }
    }
}

impl OutputHandler for State {}

impl XdgShellHandler for State {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        // Initial-configure trap: the client cannot map the toplevel until it
        // acks a configure. Suggest the output size as initial geometry, send
        // the configure, then track the window.
        surface.with_pending_state(|state| {
            state.size = Some((self.config.width as i32, self.config.height as i32).into());
        });
        surface.send_configure();
        self.window_manager.register(surface);
    }

    fn new_popup(&mut self, _surface: PopupSurface, _positioner: PositionerState) {
        // Popups are not supported in M3; nothing to configure.
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {}

    fn reposition_request(
        &mut self,
        _surface: PopupSurface,
        _positioner: PositionerState,
        _token: u32,
    ) {
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        self.window_manager.destroy(surface.wl_surface());
    }

    fn ack_configure(&mut self, surface: WlSurface, _configure: Configure) {
        self.window_manager.mark_acked(&surface);
    }
}

impl SelectionHandler for State {
    type SelectionUserData = ();
}

impl ClientDndGrabHandler for State {}
impl ServerDndGrabHandler for State {}

impl DataDeviceHandler for State {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

delegate_compositor!(State);
delegate_shm!(State);
delegate_seat!(State);
delegate_output!(State);
delegate_xdg_shell!(State);
delegate_viewporter!(State);
delegate_data_device!(State);
delegate_text_input_manager!(State);
delegate_dmabuf!(State);

// Legacy wl_shell (hand-rolled state in crate::wl_shell).
delegate_global_dispatch!(State: [wl_shell::WlShell: ()] => crate::wl_shell::WlShellState);
delegate_dispatch!(State: [wl_shell::WlShell: ()] => crate::wl_shell::WlShellState);
delegate_dispatch!(
    State: [
        wl_shell_surface::WlShellSurface: crate::wl_shell::WlShellSurfaceData
    ] => crate::wl_shell::WlShellState
);

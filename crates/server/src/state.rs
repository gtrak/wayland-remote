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

use smithay::delegate_compositor;
use smithay::delegate_output;
use smithay::delegate_seat;
use smithay::delegate_shm;
use smithay::delegate_xdg_shell;
use smithay::input::keyboard::XkbConfig;
use smithay::input::pointer::CursorImageStatus;
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::utils::{Point, Serial, Size, Transform};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    BufferAssignment, CompositorClientState, CompositorHandler, CompositorState, SurfaceAttributes,
    with_states,
};
use smithay::wayland::output::{OutputHandler, OutputManagerState};
use smithay::wayland::shell::xdg::{
    Configure, PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
    XdgToplevelSurfaceData,
};
use smithay::wayland::shm::{ShmHandler, ShmState, with_buffer_contents};
use wayland_remote_protocol::{Compression, InputEvent};
use wayland_server::backend::{ClientData, ObjectId};
use wayland_server::protocol::wl_buffer::WlBuffer;
use wayland_server::protocol::wl_seat;
use wayland_server::protocol::wl_surface::WlSurface;
use wayland_server::{Client, DisplayHandle, Resource};

use crate::rendering::{FrameBuffer, OffscreenRenderer, RenderRequest};

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

    /// Record a streamed frame of `bytes` payload size.
    pub fn record_frame(&mut self, bytes: usize) {
        self.frames_total += 1;
        self.frames_this_second += 1;
        self.frame_bytes_total += bytes as u64;
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
        let frames_per_sec = if self.second_start.elapsed() >= Duration::from_secs(1) {
            let fps = self.frames_this_second;
            self.frames_this_second = 0;
            self.second_start = Instant::now();
            fps
        } else {
            0
        };
        TelemetrySnapshot {
            frames_per_sec,
            frames_total: self.frames_total,
            frame_bytes_total: self.frame_bytes_total,
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
    /// Tracks xdg toplevels, window ids, focus, and pending window events.
    pub window_manager: crate::window::WindowManager,
    /// Committed surfaces, keyed by object id, with buffer + layout position.
    pub surfaces: HashMap<ObjectId, SurfaceInfo>,
    /// Offscreen renderer, initialized after display setup.
    pub renderer: Option<OffscreenRenderer>,
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
}

impl State {
    /// Build the full state: compositor, shm, seat (keyboard + pointer), and output.
    pub fn new(
        display_handle: DisplayHandle,
        config: Config,
        status_tx: Option<Sender<usize>>,
        render_rx: Option<Receiver<RenderRequest>>,
        shutdown: Arc<AtomicBool>,
    ) -> anyhow::Result<Self> {
        let compositor_state = CompositorState::new::<State>(&display_handle);
        let shm_state = ShmState::new::<State>(&display_handle, vec![]);
        let xdg_shell_state = XdgShellState::new::<State>(&display_handle);

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

    /// Render a single mapped window's committed buffer at its current size.
    pub fn render_window(&mut self, window_id: u64) -> anyhow::Result<FrameBuffer> {
        let (width, height) = self
            .window_manager
            .window_size(window_id)
            .ok_or_else(|| anyhow::anyhow!("window {window_id} not mapped"))?;
        let surface_id = self
            .window_manager
            .surface_id_for(window_id)
            .ok_or_else(|| anyhow::anyhow!("window {window_id} not found"))?
            .clone();
        let info = self
            .surfaces
            .get(&surface_id)
            .ok_or_else(|| anyhow::anyhow!("no surface info for window {window_id}"))?;
        let buffer = info
            .buffer
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("window {window_id} has no committed buffer"))?;
        let renderer = self
            .renderer
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("no offscreen renderer configured"))?;
        let mut frame = renderer.render_surface(buffer, width, height)?;
        frame.window_id = window_id;
        Ok(frame)
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
        let committed = with_states(surface, |states| {
            let mut guard = states.cached_state.get::<SurfaceAttributes>();
            let attrs = guard.current();
            match attrs.buffer {
                Some(BufferAssignment::NewBuffer(ref buffer)) => {
                    let (width, height) =
                        with_buffer_contents(buffer, |_, _, data| (data.width, data.height))
                            .ok()
                            .unwrap_or((0, 0));
                    let width = u32::try_from(width).unwrap_or(0);
                    let height = u32::try_from(height).unwrap_or(0);
                    Some((buffer.clone(), width, height))
                }
                _ => None,
            }
        });
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

impl SeatHandler for State {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<State> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, _seat: &Seat<State>, _focused: Option<&WlSurface>) {}

    fn cursor_image(&mut self, _seat: &Seat<State>, _image: CursorImageStatus) {}
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
        self.window_manager.destroy(&surface);
    }

    fn ack_configure(&mut self, surface: WlSurface, _configure: Configure) {
        self.window_manager.mark_acked(&surface);
    }
}

delegate_compositor!(State);
delegate_shm!(State);
delegate_seat!(State);
delegate_output!(State);
delegate_xdg_shell!(State);

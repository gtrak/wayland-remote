//! Shared state and protocol handler wiring for the headless compositor.
//!
//! [`State`] owns the Wayland protocol state (compositor, shm, seat, output),
//! tracks committed surfaces by object id, and reports surface-count changes
//! over an optional status channel. The `delegate_*!` macros forward
//! protocol dispatch to the per-global state fields.

use std::borrow::Cow;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;

use smithay::backend::input::KeyState;
use smithay::delegate_compositor;
use smithay::delegate_output;
use smithay::delegate_seat;
use smithay::delegate_shm;
use smithay::input::keyboard::{KeyboardTarget, KeysymHandle, ModifiersState, XkbConfig};
use smithay::input::pointer::{
    AxisFrame, ButtonEvent, CursorImageStatus, GestureHoldBeginEvent, GestureHoldEndEvent,
    GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent, GestureSwipeBeginEvent,
    GestureSwipeEndEvent, GestureSwipeUpdateEvent, MotionEvent, PointerTarget, RelativeMotionEvent,
};
use smithay::input::touch::{
    DownEvent, MotionEvent as TouchMotionEvent, OrientationEvent, ShapeEvent, TouchTarget, UpEvent,
};
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::output::{Mode, Output, PhysicalProperties, Scale, Subpixel};
use smithay::utils::{IsAlive, Point, Serial, Size, Transform};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{
    BufferAssignment, CompositorClientState, CompositorHandler, CompositorState, SurfaceAttributes,
    with_states,
};
use smithay::wayland::output::{OutputHandler, OutputManagerState};
use smithay::wayland::seat::WaylandFocus;
use smithay::wayland::shm::{ShmHandler, ShmState, with_buffer_contents};
use wayland_server::backend::{ClientData, ObjectId};
use wayland_server::protocol::wl_buffer::WlBuffer;
use wayland_server::protocol::wl_surface::WlSurface;
use wayland_server::{Client, DisplayHandle, Resource};

/// Configuration for the headless compositor.
#[derive(Clone, Debug)]
pub struct Config {
    /// Output width in pixels.
    pub width: u32,
    /// Output height in pixels.
    pub height: u32,
    /// Socket name to bind (created under `$XDG_RUNTIME_DIR`); auto-named if `None`.
    pub socket_name: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            width: 1280,
            height: 720,
            socket_name: None,
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

/// Focus target for the headless seat.
///
/// Wraps the object id of a `WlSurface`. No focus is ever set at this
/// milestone (no rendering or input handling yet); the type exists to
/// satisfy the seat handler's focus bounds and will be driven by input
/// injection in a later issue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SurfaceFocus(pub ObjectId);

impl IsAlive for SurfaceFocus {
    fn alive(&self) -> bool {
        true
    }
}

impl WaylandFocus for SurfaceFocus {
    fn wl_surface(&self) -> Option<Cow<'_, WlSurface>> {
        None
    }
}

impl KeyboardTarget<State> for SurfaceFocus {
    fn enter(
        &self,
        _seat: &Seat<State>,
        _data: &mut State,
        _keys: Vec<KeysymHandle<'_>>,
        _serial: Serial,
    ) {
    }
    fn leave(&self, _seat: &Seat<State>, _data: &mut State, _serial: Serial) {}
    fn key(
        &self,
        _seat: &Seat<State>,
        _data: &mut State,
        _key: KeysymHandle<'_>,
        _state: KeyState,
        _serial: Serial,
        _time: u32,
    ) {
    }
    fn modifiers(
        &self,
        _seat: &Seat<State>,
        _data: &mut State,
        _modifiers: ModifiersState,
        _serial: Serial,
    ) {
    }
}

impl PointerTarget<State> for SurfaceFocus {
    fn enter(&self, _seat: &Seat<State>, _data: &mut State, _event: &MotionEvent) {}
    fn motion(&self, _seat: &Seat<State>, _data: &mut State, _event: &MotionEvent) {}
    fn relative_motion(
        &self,
        _seat: &Seat<State>,
        _data: &mut State,
        _event: &RelativeMotionEvent,
    ) {
    }
    fn button(&self, _seat: &Seat<State>, _data: &mut State, _event: &ButtonEvent) {}
    fn axis(&self, _seat: &Seat<State>, _data: &mut State, _frame: AxisFrame) {}
    fn frame(&self, _seat: &Seat<State>, _data: &mut State) {}
    fn gesture_swipe_begin(
        &self,
        _seat: &Seat<State>,
        _data: &mut State,
        _event: &GestureSwipeBeginEvent,
    ) {
    }
    fn gesture_swipe_update(
        &self,
        _seat: &Seat<State>,
        _data: &mut State,
        _event: &GestureSwipeUpdateEvent,
    ) {
    }
    fn gesture_swipe_end(
        &self,
        _seat: &Seat<State>,
        _data: &mut State,
        _event: &GestureSwipeEndEvent,
    ) {
    }
    fn gesture_pinch_begin(
        &self,
        _seat: &Seat<State>,
        _data: &mut State,
        _event: &GesturePinchBeginEvent,
    ) {
    }
    fn gesture_pinch_update(
        &self,
        _seat: &Seat<State>,
        _data: &mut State,
        _event: &GesturePinchUpdateEvent,
    ) {
    }
    fn gesture_pinch_end(
        &self,
        _seat: &Seat<State>,
        _data: &mut State,
        _event: &GesturePinchEndEvent,
    ) {
    }
    fn gesture_hold_begin(
        &self,
        _seat: &Seat<State>,
        _data: &mut State,
        _event: &GestureHoldBeginEvent,
    ) {
    }
    fn gesture_hold_end(
        &self,
        _seat: &Seat<State>,
        _data: &mut State,
        _event: &GestureHoldEndEvent,
    ) {
    }
    fn leave(&self, _seat: &Seat<State>, _data: &mut State, _serial: Serial, _time: u32) {}
}

impl TouchTarget<State> for SurfaceFocus {
    fn down(&self, _seat: &Seat<State>, _data: &mut State, _event: &DownEvent, _seq: Serial) {}
    fn up(&self, _seat: &Seat<State>, _data: &mut State, _event: &UpEvent, _seq: Serial) {}
    fn motion(
        &self,
        _seat: &Seat<State>,
        _data: &mut State,
        _event: &TouchMotionEvent,
        _seq: Serial,
    ) {
    }
    fn frame(&self, _seat: &Seat<State>, _data: &mut State, _seq: Serial) {}
    fn cancel(&self, _seat: &Seat<State>, _data: &mut State, _seq: Serial) {}
    fn shape(&self, _seat: &Seat<State>, _data: &mut State, _event: &ShapeEvent, _seq: Serial) {}
    fn orientation(
        &self,
        _seat: &Seat<State>,
        _data: &mut State,
        _event: &OrientationEvent,
        _seq: Serial,
    ) {
    }
}

/// All compositor state: globals, tracked surfaces, and shutdown plumbing.
pub struct State {
    pub display_handle: DisplayHandle,
    pub compositor_state: CompositorState,
    pub shm_state: ShmState,
    pub seat_state: SeatState<State>,
    pub seat: Seat<State>,
    pub output: Output,
    pub output_manager_state: OutputManagerState,
    /// Surfaces that have committed at least once, keyed by object id.
    pub surfaces: HashMap<ObjectId, ()>,
    pub config: Config,
    /// Test back-channel reporting the current surface count.
    pub status_tx: Option<Sender<usize>>,
    /// Set by the signal source (or externally) to request shutdown.
    pub shutdown: Arc<AtomicBool>,
}

impl State {
    /// Build the full state: compositor, shm, seat (keyboard + pointer), and output.
    pub fn new(
        display_handle: DisplayHandle,
        config: Config,
        status_tx: Option<Sender<usize>>,
        shutdown: Arc<AtomicBool>,
    ) -> anyhow::Result<Self> {
        let compositor_state = CompositorState::new::<State>(&display_handle);
        let shm_state = ShmState::new::<State>(&display_handle, vec![]);

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
            seat_state,
            seat,
            output,
            output_manager_state,
            surfaces: HashMap::new(),
            config,
            status_tx,
            shutdown,
        })
    }

    /// Number of tracked committed surfaces.
    #[must_use]
    pub fn surface_count(&self) -> usize {
        self.surfaces.len()
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
        let buffer_dims = with_states(surface, |states| {
            let mut guard = states.cached_state.get::<SurfaceAttributes>();
            let attrs = guard.current();
            match attrs.buffer {
                Some(BufferAssignment::NewBuffer(ref buffer)) => {
                    with_buffer_contents(buffer, |_, _, data| (data.width, data.height)).ok()
                }
                _ => None,
            }
        });

        self.surfaces.insert(surface.id(), ());
        self.report_surface_count();

        tracing::debug!(?buffer_dims, "surface commit");
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
    type KeyboardFocus = SurfaceFocus;
    type PointerFocus = SurfaceFocus;
    type TouchFocus = SurfaceFocus;

    fn seat_state(&mut self) -> &mut SeatState<State> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, _seat: &Seat<State>, _focused: Option<&SurfaceFocus>) {}

    fn cursor_image(&mut self, _seat: &Seat<State>, _image: CursorImageStatus) {}
}

impl OutputHandler for State {}

delegate_compositor!(State);
delegate_shm!(State);
delegate_seat!(State);
delegate_output!(State);

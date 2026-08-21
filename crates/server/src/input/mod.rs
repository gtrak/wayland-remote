//! Input injection: routes network InputEvents into the smithay seat.

pub mod keymap;

use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;

use smithay::backend::input::{Axis, AxisSource, ButtonState as SmithayButtonState, KeyState};
use smithay::input::keyboard::{FilterResult, Keycode};
use smithay::input::pointer::{AxisFrame, ButtonEvent, MotionEvent};
use smithay::utils::{Logical, Point, Serial};
use wayland_remote_protocol::{ButtonState as ProtoButtonState, InputEvent};

use crate::state::State;

/// Maintains a monotonically increasing serial counter and a millisecond
/// time base for injected input events. Lives inside [`State`] but does not
/// borrow it — the actual injection is done by the free function [`inject`].
pub struct InputRouter {
    serial: AtomicU32,
    start: Instant,
}

impl InputRouter {
    /// Create a router with a fresh serial counter (starting at 1) and time base.
    pub fn new() -> Self {
        Self {
            serial: AtomicU32::new(1),
            start: Instant::now(),
        }
    }

    pub fn next_serial(&self) -> Serial {
        Serial::from(self.serial.fetch_add(1, Ordering::Relaxed))
    }

    pub fn now_ms(&self) -> u32 {
        self.start.elapsed().as_millis() as u32
    }
}

impl Default for InputRouter {
    fn default() -> Self {
        Self::new()
    }
}

/// Inject a network input event into the smithay seat.
///
/// Free function so it can borrow `&mut State` without conflicting with
/// `InputRouter` (which lives inside `State`).
pub fn inject(state: &mut State, window_id: u64, event: InputEvent, serial: Serial, time: u32) {
    match event {
        InputEvent::KeyDown { scancode } => {
            if let Some(keycode) = keymap::scancode_to_keycode(scancode) {
                if let Some(kbd) = state.seat.get_keyboard() {
                    let _ = kbd.input(
                        state,
                        Keycode::new(keycode),
                        KeyState::Pressed,
                        serial,
                        time,
                        |_, _, _| -> FilterResult<()> { FilterResult::Forward },
                    );
                }
            } else {
                tracing::debug!(scancode, "unmapped scancode dropped");
            }
        }
        InputEvent::KeyUp { scancode } => {
            if let Some(keycode) = keymap::scancode_to_keycode(scancode) {
                if let Some(kbd) = state.seat.get_keyboard() {
                    let _ = kbd.input(
                        state,
                        Keycode::new(keycode),
                        KeyState::Released,
                        serial,
                        time,
                        |_, _, _| -> FilterResult<()> { FilterResult::Forward },
                    );
                }
            } else {
                tracing::debug!(scancode, "unmapped scancode dropped");
            }
        }
        InputEvent::PointerMove { x, y } => {
            if let Some(ptr) = state.seat.get_pointer() {
                // Resolve the target window's surface and pass it as the
                // pointer focus. Each window is its own coordinate space
                // (surface origin (0,0)), so the global focus origin is (0,0)
                // and the event location is surface-local (x, y).
                let focus = state
                    .window_manager
                    .surface_for(window_id)
                    .map(|surface| (surface.clone(), Point::<f64, Logical>::new(0.0, 0.0)));
                ptr.motion(
                    state,
                    focus,
                    &MotionEvent {
                        location: Point::<f64, Logical>::from((x, y)),
                        serial,
                        time,
                    },
                );
                // Send a frame to delimit the event group — real toolkit
                // clients (wl_pointer v5+) buffer events until a frame arrives.
                ptr.frame(state);
            }
        }
        InputEvent::PointerButton {
            button,
            state: btn_state,
        } => {
            if let Some(ptr) = state.seat.get_pointer() {
                let s = match btn_state {
                    ProtoButtonState::Pressed => SmithayButtonState::Pressed,
                    ProtoButtonState::Released => SmithayButtonState::Released,
                };
                ptr.button(
                    state,
                    &ButtonEvent {
                        serial,
                        time,
                        button,
                        state: s,
                    },
                );
                ptr.frame(state);
            }
        }
        InputEvent::Axis { dx, dy } => {
            if let Some(ptr) = state.seat.get_pointer() {
                let mut frame = AxisFrame::new(time).source(AxisSource::Wheel);
                if dx != 0.0 {
                    frame = frame.value(Axis::Horizontal, dx * 15.0);
                }
                if dy != 0.0 {
                    frame = frame.value(Axis::Vertical, dy * 15.0);
                }
                ptr.axis(state, frame);
                ptr.frame(state);
            }
        }
    }
}

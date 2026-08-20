//! Input event construction from Win32 message parameters.
//!
//! These are pure functions operating on raw integer values (lParam, wParam,
//! message IDs) so they can be unit-tested on any platform. The actual Win32
//! constants (WM_KEYDOWN etc.) are passed as u32 from the display layer.

use wayland_remote_protocol::{ButtonState, InputEvent};

/// Extract the scancode and extended-key flag from an WM_KEYDOWN/WM_KEYUP lParam.
/// Scancode is in bits 16-23; extended flag is bit 24 (KF_EXTENDED).
pub fn extract_scancode(lparam: usize) -> (u16, bool) {
    let scancode = ((lparam >> 16) & 0xFF) as u16;
    let is_extended = (lparam & (1 << 24)) != 0;
    (scancode, is_extended)
}

/// Build a KeyDown or KeyUp event from the extracted scancode.
/// Extended keys get an offset of 0x100 to distinguish them.
pub fn key_event(scancode: u16, is_extended: bool, pressed: bool) -> InputEvent {
    let code = if is_extended {
        scancode | 0x100
    } else {
        scancode
    };
    if pressed {
        InputEvent::KeyDown { scancode: code }
    } else {
        InputEvent::KeyUp { scancode: code }
    }
}

/// Extract pointer x,y from WM_MOUSEMOVE/WM_LBUTTON* lParam (GET_X_LPARAM, GET_Y_LPARAM).
pub fn pointer_move(lparam: i32) -> (f64, f64) {
    let x = (lparam as u16) as f64;
    let y = ((lparam >> 16) as u16) as f64;
    (x, y)
}

/// Map a Win32 mouse button message to a (linux button code, state) pair.
/// Returns None for non-button messages.
/// WM_LBUTTONDOWN=0x0201, WM_LBUTTONUP=0x0202,
/// WM_RBUTTONDOWN=0x0204, WM_RBUTTONUP=0x0205,
/// WM_MBUTTONDOWN=0x0207, WM_MBUTTONUP=0x0208,
/// WM_XBUTTONDOWN=0x020B, WM_XBUTTONUP=0x020C
pub fn pointer_button(msg: u32, wparam: usize) -> Option<(u32, ButtonState)> {
    let state = match msg {
        0x0201 | 0x0204 | 0x0207 | 0x020B => ButtonState::Pressed,
        0x0202 | 0x0205 | 0x0208 | 0x020C => ButtonState::Released,
        _ => return None,
    };
    let button = match msg {
        0x0201 | 0x0202 => 0x110, // BTN_LEFT
        0x0204 | 0x0205 => 0x111, // BTN_RIGHT
        0x0207 | 0x0208 => 0x112, // BTN_MIDDLE
        0x020B | 0x020C => {
            // X buttons: wparam HIWORD indicates XBUTTON1 (1) or XBUTTON2 (2)
            let xbutton = (wparam >> 16) as u16;
            match xbutton {
                1 => 0x113, // BTN_SIDE
                2 => 0x114, // BTN_EXTRA
                _ => return None,
            }
        }
        _ => return None,
    };
    Some((button, state))
}

/// Extract scroll deltas from WM_MOUSEWHEEL (0x020A) or WM_MOUSEHWHEEL (0x020E).
/// WM_MOUSEWHEEL: wparam HIWORD = delta (positive = up). WM_MOUSEHWHEEL: positive = right.
pub fn scroll(msg: u32, wparam: usize) -> (f64, f64) {
    let delta = ((wparam >> 16) as i16) as f64 / 120.0;
    match msg {
        0x020A => (0.0, -delta), // vertical: positive delta = scroll up = negative dy (toward user)
        0x020E => (delta, 0.0),  // horizontal
        _ => (0.0, 0.0),
    }
}

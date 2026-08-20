//! Unit tests for the pure Win32 -> protocol input translation functions.

use wayland_remote_protocol::{ButtonState, InputEvent};
use wayland_remote_viewer::input::*;

// @lat: [[tests#Viewer#Scancode extraction]]
#[test]
fn scancode_extraction() {
    // lParam with scancode 0x1E (A key) in bits 16-23, no extended flag
    let lparam = (0x1Eu32 << 16) as usize;
    let (sc, ext) = extract_scancode(lparam);
    assert_eq!(sc, 0x1E);
    assert!(!ext);

    // Extended flag set (bit 24)
    let lparam = ((0x1Du32 << 16) | (1 << 24)) as usize;
    let (sc, ext) = extract_scancode(lparam);
    assert_eq!(sc, 0x1D);
    assert!(ext);
}

// @lat: [[tests#Viewer#Button mapping]]
#[test]
fn button_mapping() {
    // WM_LBUTTONDOWN = 0x0201
    let result = pointer_button(0x0201, 0);
    assert_eq!(result, Some((0x110, ButtonState::Pressed)));

    // WM_LBUTTONUP = 0x0202
    let result = pointer_button(0x0202, 0);
    assert_eq!(result, Some((0x110, ButtonState::Released)));

    // WM_RBUTTONDOWN = 0x0204
    let result = pointer_button(0x0204, 0);
    assert_eq!(result, Some((0x111, ButtonState::Pressed)));

    // WM_MBUTTONDOWN = 0x0207
    let result = pointer_button(0x0207, 0);
    assert_eq!(result, Some((0x112, ButtonState::Pressed)));

    // Non-button message
    assert_eq!(pointer_button(0x0200, 0), None); // WM_MOUSEMOVE
}

// @lat: [[tests#Viewer#Scroll direction]]
#[test]
fn scroll_direction() {
    // WM_MOUSEWHEEL = 0x020A, positive delta (up) = wparam HIWORD = 120
    let (dx, dy) = scroll(0x020A, (120u32 << 16) as usize);
    assert_eq!(dx, 0.0);
    assert!(
        dy < 0.0,
        "positive wheel delta should produce negative dy (scroll up)"
    );

    // Negative delta (down)
    let (_dx, dy) = scroll(0x020A, ((-120i16 as u32) << 16) as usize);
    assert!(
        dy > 0.0,
        "negative wheel delta should produce positive dy (scroll down)"
    );
}

// @lat: [[tests#Viewer#Key event construction]]
#[test]
fn key_event_construction() {
    let ev = key_event(0x1E, false, true);
    assert!(matches!(ev, InputEvent::KeyDown { scancode: 0x1E }));

    let ev = key_event(0x1E, false, false);
    assert!(matches!(ev, InputEvent::KeyUp { scancode: 0x1E }));

    // Extended key gets 0x100 offset
    let ev = key_event(0x1D, true, true);
    assert!(matches!(ev, InputEvent::KeyDown { scancode: 0x11D }));
}

//! Input event capture for Windows viewer
//!
//! Captures keyboard and mouse events from winit windows and encodes
//! them for transmission to the Linux compositor server.

use std::sync::mpsc;
use tracing::{debug, info};
use winit::event::{ElementState, KeyEvent as WinitKeyEvent, MouseButton, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// Input event types for transmission to server
#[derive(Debug, Clone, PartialEq)]
pub enum ViewerInputEvent {
    /// Key press event
    KeyPress { key_code: u32, modifiers: u8 },
    /// Key release event
    KeyRelease { key_code: u32, modifiers: u8 },
    /// Mouse move event (relative to window)
    MouseMove { x: i32, y: i32 },
    /// Mouse button press
    MouseButtonPress { button: u8 },
    /// Mouse button release
    MouseButtonRelease { button: u8 },
    /// Mouse scroll
    MouseScroll { horizontal: i32, vertical: i32 },
}

/// Input event with window ID
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInputEvent {
    /// Window ID from the compositor
    pub window_id: u32,
    /// The input event
    pub event: ViewerInputEvent,
}

/// Input capture state for a window
#[derive(Debug)]
pub struct InputCapture {
    /// Channel for sending input events
    event_sender: Option<mpsc::Sender<WindowInputEvent>>,
    /// Current modifier state
    modifiers: u8,
}

impl InputCapture {
    /// Create a new input capture instance
    pub fn new() -> Self {
        Self {
            event_sender: None,
            modifiers: 0,
        }
    }

    /// Set the event sender channel
    pub fn set_sender(&mut self, sender: mpsc::Sender<WindowInputEvent>) {
        self.event_sender = Some(sender);
    }

    /// Handle a winit window event
    ///
    /// Returns true if the event was consumed (should not be processed further)
    pub fn handle_event(&mut self, window_id: u32, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput { event, .. } => {
                self.handle_keyboard_input(window_id, event);
                true
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.handle_mouse_move(window_id, position.x as i32, position.y as i32);
                true
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_mouse_button(window_id, *state, *button);
                true
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.handle_mouse_wheel(window_id, delta);
                true
            }
            _ => false,
        }
    }

    /// Handle keyboard input
    fn handle_keyboard_input(&mut self, window_id: u32, event: &WinitKeyEvent) {
        let key_code = self.winit_key_to_linux_code(&event.physical_key);
        
        // Update modifiers
        self.update_modifiers(&event.physical_key, event.state == ElementState::Pressed);

        let viewer_event = match event.state {
            ElementState::Pressed => ViewerInputEvent::KeyPress {
                key_code,
                modifiers: self.modifiers,
            },
            ElementState::Released => ViewerInputEvent::KeyRelease {
                key_code,
                modifiers: self.modifiers,
            },
        };

        self.send_event(window_id, viewer_event);
    }

    /// Handle mouse move
    fn handle_mouse_move(&mut self, window_id: u32, x: i32, y: i32) {
        self.send_event(
            window_id,
            ViewerInputEvent::MouseMove { x, y },
        );
    }

    /// Handle mouse button
    fn handle_mouse_button(&mut self, window_id: u32, state: ElementState, button: MouseButton) {
        let button_num = match button {
            MouseButton::Left => 1,
            MouseButton::Right => 2,
            MouseButton::Middle => 3,
            MouseButton::Back => 4,
            MouseButton::Forward => 5,
            MouseButton::Other(n) => n as u8 + 5,
        };

        let viewer_event = match state {
            ElementState::Pressed => ViewerInputEvent::MouseButtonPress { button: button_num },
            ElementState::Released => ViewerInputEvent::MouseButtonRelease { button: button_num },
        };

        self.send_event(window_id, viewer_event);
    }

    /// Handle mouse wheel
    fn handle_mouse_wheel(&mut self, window_id: u32, delta: &winit::event::MouseScrollDelta) {
        let (horizontal, vertical) = match delta {
            winit::event::MouseScrollDelta::LineDelta(h, v) => {
                (*h as i32 * 120, *v as i32 * 120) // Convert lines to "ticks"
            }
            winit::event::MouseScrollDelta::PixelDelta(pos) => {
                (pos.x as i32, pos.y as i32)
            }
        };

        self.send_event(
            window_id,
            ViewerInputEvent::MouseScroll {
                horizontal,
                vertical,
            },
        );
    }

    /// Send an event through the channel
    fn send_event(&self, window_id: u32, event: ViewerInputEvent) {
        if let Some(ref sender) = self.event_sender {
            let window_event = WindowInputEvent { window_id, event };
            if let Err(e) = sender.send(window_event) {
                debug!(error = %e, "Failed to send input event");
            }
        }
    }

    /// Update modifier state
    fn update_modifiers(&mut self, key: &PhysicalKey, pressed: bool) {
        let modifier_bit = match key {
            PhysicalKey::Code(KeyCode::ShiftLeft) | PhysicalKey::Code(KeyCode::ShiftRight) => 0x01,
            PhysicalKey::Code(KeyCode::ControlLeft) | PhysicalKey::Code(KeyCode::ControlRight) => 0x02,
            PhysicalKey::Code(KeyCode::AltLeft) | PhysicalKey::Code(KeyCode::AltRight) => 0x04,
            PhysicalKey::Code(KeyCode::SuperLeft) | PhysicalKey::Code(KeyCode::SuperRight) => 0x08,
            _ => return,
        };

        if pressed {
            self.modifiers |= modifier_bit;
        } else {
            self.modifiers &= !modifier_bit;
        }
    }

    /// Convert winit PhysicalKey to Linux input event code
    ///
    /// This maps Windows virtual key codes to Linux input event codes.
    fn winit_key_to_linux_code(&self, key: &PhysicalKey) -> u32 {
        match key {
            PhysicalKey::Code(code) => keycode_to_linux(code),
            PhysicalKey::Unidentified(_) => 0,
        }
    }
}

impl Default for InputCapture {
    fn default() -> Self {
        Self::new()
    }
}

/// Map winit KeyCode to Linux input event code
///
/// Returns the Linux input event code for a winit KeyCode.
/// This is a partial mapping covering common keys.
fn keycode_to_linux(code: &KeyCode) -> u32 {
    use winit::keyboard::KeyCode;
    
    match code {
        KeyCode::Backspace => 14,
        KeyCode::Tab => 15,
        KeyCode::Enter => 28,
        KeyCode::ShiftLeft => 42,
        KeyCode::ShiftRight => 54,
        KeyCode::ControlLeft => 29,
        KeyCode::ControlRight => 97,
        KeyCode::AltLeft => 56,
        KeyCode::AltRight => 100,
        KeyCode::CapsLock => 58,
        KeyCode::Escape => 1,
        KeyCode::Space => 57,
        KeyCode::PageUp => 104,
        KeyCode::PageDown => 109,
        KeyCode::End => 107,
        KeyCode::Home => 102,
        KeyCode::ArrowLeft => 105,
        KeyCode::ArrowUp => 103,
        KeyCode::ArrowRight => 106,
        KeyCode::ArrowDown => 108,
        KeyCode::Insert => 110,
        KeyCode::Delete => 111,
        KeyCode::Digit0 => 11,
        KeyCode::Digit1 => 2,
        KeyCode::Digit2 => 3,
        KeyCode::Digit3 => 4,
        KeyCode::Digit4 => 5,
        KeyCode::Digit5 => 6,
        KeyCode::Digit6 => 7,
        KeyCode::Digit7 => 8,
        KeyCode::Digit8 => 9,
        KeyCode::Digit9 => 10,
        KeyCode::KeyA => 30,
        KeyCode::KeyB => 48,
        KeyCode::KeyC => 46,
        KeyCode::KeyD => 32,
        KeyCode::KeyE => 18,
        KeyCode::KeyF => 33,
        KeyCode::KeyG => 34,
        KeyCode::KeyH => 35,
        KeyCode::KeyI => 23,
        KeyCode::KeyJ => 36,
        KeyCode::KeyK => 37,
        KeyCode::KeyL => 38,
        KeyCode::KeyM => 50,
        KeyCode::KeyN => 49,
        KeyCode::KeyO => 24,
        KeyCode::KeyP => 25,
        KeyCode::KeyQ => 16,
        KeyCode::KeyR => 19,
        KeyCode::KeyS => 31,
        KeyCode::KeyT => 20,
        KeyCode::KeyU => 22,
        KeyCode::KeyV => 47,
        KeyCode::KeyW => 17,
        KeyCode::KeyX => 45,
        KeyCode::KeyY => 21,
        KeyCode::KeyZ => 44,
        KeyCode::F1 => 59,
        KeyCode::F2 => 60,
        KeyCode::F3 => 61,
        KeyCode::F4 => 62,
        KeyCode::F5 => 63,
        KeyCode::F6 => 64,
        KeyCode::F7 => 65,
        KeyCode::F8 => 66,
        KeyCode::F9 => 67,
        KeyCode::F10 => 68,
        KeyCode::F11 => 87,
        KeyCode::F12 => 88,
        KeyCode::Minus => 12,
        KeyCode::Equal => 13,
        KeyCode::BracketLeft => 26,
        KeyCode::BracketRight => 27,
        KeyCode::Backslash => 43,
        KeyCode::Semicolon => 39,
        KeyCode::Quote => 40,
        KeyCode::Comma => 51,
        KeyCode::Period => 52,
        KeyCode::Slash => 53,
        KeyCode::Backquote => 41,
        KeyCode::SuperLeft => 125,
        KeyCode::SuperRight => 126,
        KeyCode::ContextMenu => 127,
        _ => 0, // Unknown key
    }
}

/// Encode a WindowInputEvent for network transmission
///
/// Encodes the event into the protocol format expected by the server.
pub fn encode_input_event(event: &WindowInputEvent) -> Vec<u8> {
    use winit::event::MouseScrollDelta;
    
    let mut buf = Vec::with_capacity(16);
    
    // Event type (1 byte)
    let event_type = match &event.event {
        ViewerInputEvent::KeyPress { .. } => 0x01u8,
        ViewerInputEvent::KeyRelease { .. } => 0x02u8,
        ViewerInputEvent::MouseMove { .. } => 0x03u8,
        ViewerInputEvent::MouseButtonPress { .. } => 0x04u8,
        ViewerInputEvent::MouseButtonRelease { .. } => 0x05u8,
        ViewerInputEvent::MouseScroll { .. } => 0x06u8,
    };
    buf.push(event_type);
    
    // Window ID (4 bytes, big-endian)
    buf.extend_from_slice(&event.window_id.to_be_bytes());
    
    // Event data
    match &event.event {
        ViewerInputEvent::KeyPress { key_code, modifiers } |
        ViewerInputEvent::KeyRelease { key_code, modifiers } => {
            // Key code (4 bytes)
            buf.extend_from_slice(&key_code.to_be_bytes());
            // Modifiers (1 byte)
            buf.push(*modifiers);
        }
        ViewerInputEvent::MouseMove { x, y } => {
            // X (4 bytes, signed)
            buf.extend_from_slice(&x.to_be_bytes());
            // Y (4 bytes, signed)
            buf.extend_from_slice(&y.to_be_bytes());
        }
        ViewerInputEvent::MouseButtonPress { button } |
        ViewerInputEvent::MouseButtonRelease { button } => {
            // Button (1 byte)
            buf.push(*button);
        }
        ViewerInputEvent::MouseScroll { horizontal, vertical } => {
            // Horizontal (4 bytes, signed)
            buf.extend_from_slice(&horizontal.to_be_bytes());
            // Vertical (4 bytes, signed)
            buf.extend_from_slice(&vertical.to_be_bytes());
        }
    }
    
    buf
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::event::{KeyEvent, MouseScrollDelta, ElementState};
    use winit::keyboard::{PhysicalKey, KeyCode, NativeKeyCode};
    use winit::dpi::LogicalPosition;
    
    #[test]
    fn test_input_capture_new() {
        let capture = InputCapture::new();
        assert_eq!(capture.modifiers, 0);
        assert!(capture.event_sender.is_none());
    }
    
    #[test]
    fn test_keycode_to_linux() {
        // Test some common key mappings
        assert_eq!(keycode_to_linux(&KeyCode::KeyA), 30);
        assert_eq!(keycode_to_linux(&KeyCode::KeyZ), 44);
        assert_eq!(keycode_to_linux(&KeyCode::Space), 57);
        assert_eq!(keycode_to_linux(&KeyCode::Enter), 28);
        assert_eq!(keycode_to_linux(&KeyCode::Escape), 1);
        assert_eq!(keycode_to_linux(&KeyCode::ArrowUp), 103);
        assert_eq!(keycode_to_linux(&KeyCode::Digit1), 2);
        assert_eq!(keycode_to_linux(&KeyCode::F1), 59);
        assert_eq!(keycode_to_linux(&KeyCode::Unknown), 0);
    }
    
    #[test]
    fn test_encode_key_press() {
        let event = WindowInputEvent {
            window_id: 42,
            event: ViewerInputEvent::KeyPress {
                key_code: 30, // 'A'
                modifiers: 0x01, // Shift
            },
        };
        
        let encoded = encode_input_event(&event);
        
        // Event type
        assert_eq!(encoded[0], 0x01);
        // Window ID (big-endian)
        assert_eq!(u32::from_be_bytes([encoded[1], encoded[2], encoded[3], encoded[4]]), 42);
        // Key code (big-endian)
        assert_eq!(u32::from_be_bytes([encoded[5], encoded[6], encoded[7], encoded[8]]), 30);
        // Modifiers
        assert_eq!(encoded[9], 0x01);
    }
    
    #[test]
    fn test_encode_key_release() {
        let event = WindowInputEvent {
            window_id: 1,
            event: ViewerInputEvent::KeyRelease {
                key_code: 57, // Space
                modifiers: 0,
            },
        };
        
        let encoded = encode_input_event(&event);
        assert_eq!(encoded[0], 0x02);
    }
    
    #[test]
    fn test_encode_mouse_move() {
        let event = WindowInputEvent {
            window_id: 100,
            event: ViewerInputEvent::MouseMove { x: -50, y: 200 },
        };
        
        let encoded = encode_input_event(&event);
        
        // Event type
        assert_eq!(encoded[0], 0x03);
        // Window ID
        assert_eq!(u32::from_be_bytes([encoded[1], encoded[2], encoded[3], encoded[4]]), 100);
        // X coordinate (signed, big-endian)
        assert_eq!(i32::from_be_bytes([encoded[5], encoded[6], encoded[7], encoded[8]]), -50);
        // Y coordinate
        assert_eq!(i32::from_be_bytes([encoded[9], encoded[10], encoded[11], encoded[12]]), 200);
    }
    
    #[test]
    fn test_encode_mouse_button() {
        let press = WindowInputEvent {
            window_id: 1,
            event: ViewerInputEvent::MouseButtonPress { button: 1 },
        };
        
        let encoded = encode_input_event(&press);
        assert_eq!(encoded[0], 0x04);
        assert_eq!(encoded[5], 1);
        
        let release = WindowInputEvent {
            window_id: 1,
            event: ViewerInputEvent::MouseButtonRelease { button: 2 },
        };
        
        let encoded = encode_input_event(&release);
        assert_eq!(encoded[0], 0x05);
        assert_eq!(encoded[5], 2);
    }
    
    #[test]
    fn test_encode_mouse_scroll() {
        let event = WindowInputEvent {
            window_id: 5,
            event: ViewerInputEvent::MouseScroll {
                horizontal: -3,
                vertical: 5,
            },
        };
        
        let encoded = encode_input_event(&event);
        
        // Event type
        assert_eq!(encoded[0], 0x06);
        // Window ID
        assert_eq!(u32::from_be_bytes([encoded[1], encoded[2], encoded[3], encoded[4]]), 5);
        // Horizontal (signed)
        assert_eq!(i32::from_be_bytes([encoded[5], encoded[6], encoded[7], encoded[8]]), -3);
        // Vertical (signed)
        assert_eq!(i32::from_be_bytes([encoded[9], encoded[10], encoded[11], encoded[12]]), 5);
    }
    
    #[test]
    fn test_modifier_tracking() {
        let mut capture = InputCapture::new();
        
        // Initial state
        assert_eq!(capture.modifiers, 0);
        
        // Press shift
        capture.update_modifiers(&PhysicalKey::Code(KeyCode::ShiftLeft), true);
        assert_eq!(capture.modifiers, 0x01);
        
        // Press ctrl
        capture.update_modifiers(&PhysicalKey::Code(KeyCode::ControlLeft), true);
        assert_eq!(capture.modifiers, 0x03);
        
        // Release shift
        capture.update_modifiers(&PhysicalKey::Code(KeyCode::ShiftLeft), false);
        assert_eq!(capture.modifiers, 0x02);
        
        // Release ctrl
        capture.update_modifiers(&PhysicalKey::Code(KeyCode::ControlLeft), false);
        assert_eq!(capture.modifiers, 0);
    }
    
    #[test]
    fn test_mouse_button_mapping() {
        let mut capture = InputCapture::new();
        let (tx, rx) = mpsc::channel();
        capture.set_sender(tx);
        
        // Test left button
        capture.handle_mouse_button(1, ElementState::Pressed, MouseButton::Left);
        let event = rx.recv().unwrap();
        assert_eq!(event.event, ViewerInputEvent::MouseButtonPress { button: 1 });
        
        // Test right button
        capture.handle_mouse_button(1, ElementState::Released, MouseButton::Right);
        let event = rx.recv().unwrap();
        assert_eq!(event.event, ViewerInputEvent::MouseButtonRelease { button: 2 });
        
        // Test middle button
        capture.handle_mouse_button(1, ElementState::Pressed, MouseButton::Middle);
        let event = rx.recv().unwrap();
        assert_eq!(event.event, ViewerInputEvent::MouseButtonPress { button: 3 });
    }
    
    #[test]
    fn test_default_implementation() {
        let capture: InputCapture = Default::default();
        assert_eq!(capture.modifiers, 0);
    }
}

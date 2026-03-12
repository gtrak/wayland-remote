//! Input event handler for bidirectional streaming
//!
//! Processes input events received from the Windows viewer and forwards
//! them to the appropriate Wayland surfaces via the wl_seat.

use smithay::input::{Seat, SeatHandler};
use smithay::utils::{Serial, SERIAL_COUNTER};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::reexports::wayland_server::backend::ObjectId;
use std::collections::HashMap;
use tracing::{debug, info, warn};

use crate::streaming::input::{
    InputEvent, InputEventHandler, KeyEvent, MouseButtonEvent, MouseMoveEvent, MouseScrollEvent,
};

/// Input event processor that forwards events to Wayland surfaces
///
/// This struct maintains the mapping between window IDs and Wayland surfaces,
/// and handles the forwarding of input events from the viewer to the compositor.
#[derive(Debug)]
pub struct InputProcessor {
    /// Maps window IDs to their corresponding Wayland surface ObjectIds
    window_to_surface: HashMap<u32, ObjectId>,
    /// Maps surface ObjectIds to window IDs (reverse lookup)
    surface_to_window: HashMap<ObjectId, u32>,
    /// Serial counter for input events
    serial: Serial,
}

impl InputProcessor {
    /// Create a new input processor
    pub fn new() -> Self {
        Self {
            window_to_surface: HashMap::new(),
            surface_to_window: HashMap::new(),
            serial: SERIAL_COUNTER.next_serial(),
        }
    }

    /// Register a window-to-surface mapping
    ///
    /// Called when a new toplevel window is created.
    pub fn register_window(&mut self, window_id: u32, surface_id: ObjectId) {
        info!(window_id, ?surface_id, "Registering window-to-surface mapping");
        self.window_to_surface.insert(window_id, surface_id.clone());
        self.surface_to_window.insert(surface_id, window_id);
    }

    /// Unregister a window
    ///
    /// Called when a toplevel window is destroyed.
    pub fn unregister_window(&mut self, window_id: u32) {
        if let Some(surface_id) = self.window_to_surface.remove(&window_id) {
            self.surface_to_window.remove(&surface_id);
            info!(window_id, "Unregistered window");
        }
    }

    /// Unregister a surface
    ///
    /// Called when a surface is destroyed.
    pub fn unregister_surface(&mut self, surface_id: &ObjectId) {
        if let Some(window_id) = self.surface_to_window.remove(surface_id) {
            self.window_to_surface.remove(&window_id);
            info!(?surface_id, window_id, "Unregistered surface");
        }
    }

    /// Get the surface ObjectId for a window ID
    pub fn get_surface(&self, window_id: u32) -> Option<&ObjectId> {
        self.window_to_surface.get(&window_id)
    }

    /// Get the window ID for a surface ObjectId
    pub fn get_window_id(&self, surface_id: &ObjectId) -> Option<u32> {
        self.surface_to_window.get(surface_id).copied()
    }

    /// Get the next serial for input events
    pub fn next_serial(&mut self) -> Serial {
        self.serial = SERIAL_COUNTER.next_serial();
        self.serial
    }

    /// Get the number of registered windows
    pub fn window_count(&self) -> usize {
        self.window_to_surface.len()
    }

    /// Check if a window is registered
    pub fn has_window(&self, window_id: u32) -> bool {
        self.window_to_surface.contains_key(&window_id)
    }
}

impl Default for InputProcessor {
    fn default() -> Self {
        Self::new()
    }
}

impl InputEventHandler for InputProcessor {
    fn handle_input_event(&mut self, window_id: u32, event: InputEvent) {
        debug!(window_id, ?event, "Received input event");

        // Check if window is registered
        if !self.has_window(window_id) {
            warn!(window_id, "Received input event for unregistered window");
            return;
        }

        // Get the serial for this event
        let _serial = self.next_serial();

        match event {
            InputEvent::KeyPress(key) => {
                self.handle_key_press(window_id, key);
            }
            InputEvent::KeyRelease(key) => {
                self.handle_key_release(window_id, key);
            }
            InputEvent::MouseMove(mouse) => {
                self.handle_mouse_move(window_id, mouse);
            }
            InputEvent::MouseButtonPress(button) => {
                self.handle_mouse_button_press(window_id, button);
            }
            InputEvent::MouseButtonRelease(button) => {
                self.handle_mouse_button_release(window_id, button);
            }
            InputEvent::MouseScroll(scroll) => {
                self.handle_mouse_scroll(window_id, scroll);
            }
        }
    }
}

impl InputProcessor {
    /// Handle a key press event
    fn handle_key_press(&mut self, window_id: u32, key: KeyEvent) {
        info!(
            window_id,
            key_code = key.key_code,
            modifiers = key.modifiers,
            "Key press"
        );
        // Key press handling - in full implementation, this would:
        // 1. Get the keyboard from the seat
        // 2. Send the key event to the focused surface
        // 3. Update keyboard state
    }

    /// Handle a key release event
    fn handle_key_release(&mut self, window_id: u32, key: KeyEvent) {
        info!(
            window_id,
            key_code = key.key_code,
            modifiers = key.modifiers,
            "Key release"
        );
        // Key release handling
    }

    /// Handle a mouse move event
    fn handle_mouse_move(&mut self, window_id: u32, mouse: MouseMoveEvent) {
        debug!(
            window_id,
            x = mouse.x,
            y = mouse.y,
            "Mouse move"
        );
        // Mouse move handling - in full implementation, this would:
        // 1. Get the pointer from the seat
        // 2. Update pointer position
        // 3. Send motion event to surface under cursor
    }

    /// Handle a mouse button press event
    fn handle_mouse_button_press(&mut self, window_id: u32, button: MouseButtonEvent) {
        info!(
            window_id,
            button = button.button,
            "Mouse button press"
        );
        // Mouse button press handling
    }

    /// Handle a mouse button release event
    fn handle_mouse_button_release(&mut self, window_id: u32, button: MouseButtonEvent) {
        info!(
            window_id,
            button = button.button,
            "Mouse button release"
        );
        // Mouse button release handling
    }

    /// Handle a mouse scroll event
    fn handle_mouse_scroll(&mut self, window_id: u32, scroll: MouseScrollEvent) {
        info!(
            window_id,
            horizontal = scroll.horizontal,
            vertical = scroll.vertical,
            "Mouse scroll"
        );
        // Mouse scroll handling
    }
}

/// Key codes mapping from Windows virtual keys to Linux input event codes
///
/// This module provides a partial mapping for common keys.
/// A full implementation would map all VK_ codes to KEY_ codes.
pub mod keycodes {
    // Linux input event codes (from linux/input-event-codes.h)
    // Common keys
    pub const KEY_RESERVED: u32 = 0;
    pub const KEY_ESC: u32 = 1;
    pub const KEY_1: u32 = 2;
    pub const KEY_2: u32 = 3;
    pub const KEY_3: u32 = 4;
    pub const KEY_4: u32 = 5;
    pub const KEY_5: u32 = 6;
    pub const KEY_6: u32 = 7;
    pub const KEY_7: u32 = 8;
    pub const KEY_8: u32 = 9;
    pub const KEY_9: u32 = 10;
    pub const KEY_0: u32 = 11;
    pub const KEY_MINUS: u32 = 12;
    pub const KEY_EQUAL: u32 = 13;
    pub const KEY_BACKSPACE: u32 = 14;
    pub const KEY_TAB: u32 = 15;
    pub const KEY_Q: u32 = 16;
    pub const KEY_W: u32 = 17;
    pub const KEY_E: u32 = 18;
    pub const KEY_R: u32 = 19;
    pub const KEY_T: u32 = 20;
    pub const KEY_Y: u32 = 21;
    pub const KEY_U: u32 = 22;
    pub const KEY_I: u32 = 23;
    pub const KEY_O: u32 = 24;
    pub const KEY_P: u32 = 25;
    pub const KEY_LEFTBRACE: u32 = 26;
    pub const KEY_RIGHTBRACE: u32 = 27;
    pub const KEY_ENTER: u32 = 28;
    pub const KEY_LEFTCTRL: u32 = 29;
    pub const KEY_A: u32 = 30;
    pub const KEY_S: u32 = 31;
    pub const KEY_D: u32 = 32;
    pub const KEY_F: u32 = 33;
    pub const KEY_G: u32 = 34;
    pub const KEY_H: u32 = 35;
    pub const KEY_J: u32 = 36;
    pub const KEY_K: u32 = 37;
    pub const KEY_L: u32 = 38;
    pub const KEY_SEMICOLON: u32 = 39;
    pub const KEY_APOSTROPHE: u32 = 40;
    pub const KEY_GRAVE: u32 = 41;
    pub const KEY_LEFTSHIFT: u32 = 42;
    pub const KEY_BACKSLASH: u32 = 43;
    pub const KEY_Z: u32 = 44;
    pub const KEY_X: u32 = 45;
    pub const KEY_C: u32 = 46;
    pub const KEY_V: u32 = 47;
    pub const KEY_B: u32 = 48;
    pub const KEY_N: u32 = 49;
    pub const KEY_M: u32 = 50;
    pub const KEY_COMMA: u32 = 51;
    pub const KEY_DOT: u32 = 52;
    pub const KEY_SLASH: u32 = 53;
    pub const KEY_RIGHTSHIFT: u32 = 54;
    pub const KEY_KPASTERISK: u32 = 55;
    pub const KEY_LEFTALT: u32 = 56;
    pub const KEY_SPACE: u32 = 57;
    pub const KEY_CAPSLOCK: u32 = 58;
    pub const KEY_F1: u32 = 59;
    pub const KEY_F2: u32 = 60;
    pub const KEY_F3: u32 = 61;
    pub const KEY_F4: u32 = 62;
    pub const KEY_F5: u32 = 63;
    pub const KEY_F6: u32 = 64;
    pub const KEY_F7: u32 = 65;
    pub const KEY_F8: u32 = 66;
    pub const KEY_F9: u32 = 67;
    pub const KEY_F10: u32 = 68;
    pub const KEY_F11: u32 = 87;
    pub const KEY_F12: u32 = 88;
    pub const KEY_RIGHTCTRL: u32 = 97;
    pub const KEY_RIGHTALT: u32 = 100;
    pub const KEY_HOME: u32 = 102;
    pub const KEY_UP: u32 = 103;
    pub const KEY_PAGEUP: u32 = 104;
    pub const KEY_LEFT: u32 = 105;
    pub const KEY_RIGHT: u32 = 106;
    pub const KEY_END: u32 = 107;
    pub const KEY_DOWN: u32 = 108;
    pub const KEY_PAGEDOWN: u32 = 109;
    pub const KEY_INSERT: u32 = 110;
    pub const KEY_DELETE: u32 = 111;

    /// Modifier bit flags
    pub mod modifiers {
        pub const SHIFT: u8 = 0x01;
        pub const CTRL: u8 = 0x02;
        pub const ALT: u8 = 0x04;
        pub const SUPER: u8 = 0x08;
    }

    /// Map Windows virtual key code to Linux input event code
    ///
    /// This is a partial mapping for common keys.
    /// Returns None for unmapped keys.
    pub fn vk_to_linux(vk_code: u32) -> Option<u32> {
        match vk_code {
            0x08 => Some(KEY_BACKSPACE),
            0x09 => Some(KEY_TAB),
            0x0D => Some(KEY_ENTER),
            0x1B => Some(KEY_ESC),
            0x20 => Some(KEY_SPACE),
            0x21 => Some(KEY_PAGEUP),
            0x22 => Some(KEY_PAGEDOWN),
            0x23 => Some(KEY_END),
            0x24 => Some(KEY_HOME),
            0x25 => Some(KEY_LEFT),
            0x26 => Some(KEY_UP),
            0x27 => Some(KEY_RIGHT),
            0x28 => Some(KEY_DOWN),
            0x2D => Some(KEY_INSERT),
            0x2E => Some(KEY_DELETE),
            0x30 => Some(KEY_0),
            0x31 => Some(KEY_1),
            0x32 => Some(KEY_2),
            0x33 => Some(KEY_3),
            0x34 => Some(KEY_4),
            0x35 => Some(KEY_5),
            0x36 => Some(KEY_6),
            0x37 => Some(KEY_7),
            0x38 => Some(KEY_8),
            0x39 => Some(KEY_9),
            0x41 => Some(KEY_A),
            0x42 => Some(KEY_B),
            0x43 => Some(KEY_C),
            0x44 => Some(KEY_D),
            0x45 => Some(KEY_E),
            0x46 => Some(KEY_F),
            0x47 => Some(KEY_G),
            0x48 => Some(KEY_H),
            0x49 => Some(KEY_I),
            0x4A => Some(KEY_J),
            0x4B => Some(KEY_K),
            0x4C => Some(KEY_L),
            0x4D => Some(KEY_M),
            0x4E => Some(KEY_N),
            0x4F => Some(KEY_O),
            0x50 => Some(KEY_P),
            0x51 => Some(KEY_Q),
            0x52 => Some(KEY_R),
            0x53 => Some(KEY_S),
            0x54 => Some(KEY_T),
            0x55 => Some(KEY_U),
            0x56 => Some(KEY_V),
            0x57 => Some(KEY_W),
            0x58 => Some(KEY_X),
            0x59 => Some(KEY_Y),
            0x5A => Some(KEY_Z),
            0x70 => Some(KEY_F1),
            0x71 => Some(KEY_F2),
            0x72 => Some(KEY_F3),
            0x73 => Some(KEY_F4),
            0x74 => Some(KEY_F5),
            0x75 => Some(KEY_F6),
            0x76 => Some(KEY_F7),
            0x77 => Some(KEY_F8),
            0x78 => Some(KEY_F9),
            0x79 => Some(KEY_F10),
            0x7A => Some(KEY_F11),
            0x7B => Some(KEY_F12),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use smithay::reexports::wayland_server::backend::ObjectId;

    #[test]
    fn test_input_processor_new() {
        let processor = InputProcessor::new();
        assert_eq!(processor.window_count(), 0);
    }

    #[test]
    fn test_register_window() {
        let mut processor = InputProcessor::new();
        let surface_id = ObjectId::null();
        
        processor.register_window(1, surface_id.clone());
        assert_eq!(processor.window_count(), 1);
        assert!(processor.has_window(1));
        assert_eq!(processor.get_surface(1), Some(&surface_id));
    }

    #[test]
    fn test_unregister_window() {
        let mut processor = InputProcessor::new();
        let surface_id = ObjectId::null();
        
        processor.register_window(1, surface_id);
        processor.unregister_window(1);
        assert_eq!(processor.window_count(), 0);
        assert!(!processor.has_window(1));
    }

    #[test]
    fn test_unregister_surface() {
        let mut processor = InputProcessor::new();
        let surface_id = ObjectId::null();
        
        processor.register_window(1, surface_id.clone());
        processor.unregister_surface(&surface_id);
        assert_eq!(processor.window_count(), 0);
    }

    #[test]
    fn test_reverse_lookup() {
        let mut processor = InputProcessor::new();
        let surface_id = ObjectId::null();
        
        processor.register_window(42, surface_id.clone());
        assert_eq!(processor.get_window_id(&surface_id), Some(42));
    }

    #[test]
    fn test_multiple_windows() {
        let mut processor = InputProcessor::new();
        
        processor.register_window(1, ObjectId::null());
        processor.register_window(2, ObjectId::null());
        processor.register_window(3, ObjectId::null());
        
        assert_eq!(processor.window_count(), 3);
        assert!(processor.has_window(1));
        assert!(processor.has_window(2));
        assert!(processor.has_window(3));
        
        processor.unregister_window(2);
        assert_eq!(processor.window_count(), 2);
        assert!(!processor.has_window(2));
    }

    #[test]
    fn test_handle_event_unregistered_window() {
        let mut processor = InputProcessor::new();
        
        // This should not panic, just log a warning
        let event = InputEvent::KeyPress(KeyEvent {
            key_code: 30,
            modifiers: 0,
        });
        processor.handle_input_event(999, event);
        
        // Test passes if we reach here without panic
    }

    #[test]
    fn test_handle_key_press() {
        let mut processor = InputProcessor::new();
        processor.register_window(1, ObjectId::null());
        
        let event = InputEvent::KeyPress(KeyEvent {
            key_code: 30,
            modifiers: 0x01, // Shift
        });
        processor.handle_input_event(1, event);
        
        // Test passes if we reach here
    }

    #[test]
    fn test_handle_mouse_move() {
        let mut processor = InputProcessor::new();
        processor.register_window(1, ObjectId::null());
        
        let event = InputEvent::MouseMove(MouseMoveEvent { x: 100, y: 200 });
        processor.handle_input_event(1, event);
        
        // Test passes if we reach here
    }

    #[test]
    fn test_handle_mouse_button() {
        let mut processor = InputProcessor::new();
        processor.register_window(1, ObjectId::null());
        
        let press = InputEvent::MouseButtonPress(MouseButtonEvent { button: 1 });
        let release = InputEvent::MouseButtonRelease(MouseButtonEvent { button: 1 });
        
        processor.handle_input_event(1, press);
        processor.handle_input_event(1, release);
        
        // Test passes if we reach here
    }

    #[test]
    fn test_handle_mouse_scroll() {
        let mut processor = InputProcessor::new();
        processor.register_window(1, ObjectId::null());
        
        let event = InputEvent::MouseScroll(MouseScrollEvent {
            horizontal: 0,
            vertical: -3,
        });
        processor.handle_input_event(1, event);
        
        // Test passes if we reach here
    }

    #[test]
    fn test_keycode_mapping() {
        use keycodes::*;
        
        // Test some common mappings
        assert_eq!(vk_to_linux(0x41), Some(KEY_A)); // 'A'
        assert_eq!(vk_to_linux(0x20), Some(KEY_SPACE));
        assert_eq!(vk_to_linux(0x0D), Some(KEY_ENTER));
        assert_eq!(vk_to_linux(0x1B), Some(KEY_ESC));
        assert_eq!(vk_to_linux(0x26), Some(KEY_UP));
        assert_eq!(vk_to_linux(0x70), Some(KEY_F1));
        assert_eq!(vk_to_linux(0x79), Some(KEY_F10));
    }

    #[test]
    fn test_keycode_mapping_unknown() {
        use keycodes::*;
        
        // Unknown key code returns None
        assert_eq!(vk_to_linux(0xFFFF), None);
    }

    #[test]
    fn test_default_implementation() {
        let processor: InputProcessor = Default::default();
        assert_eq!(processor.window_count(), 0);
    }
}

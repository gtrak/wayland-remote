//! Input event protocol for bidirectional streaming
//!
//! Defines the binary protocol for sending input events from the viewer
//! back to the Wayland compositor. This enables keyboard and mouse input
//! to be forwarded from the Windows client to the Linux server.
//!
//! Protocol format:
//! - Event type: u8 (1 byte)
//! - Window ID: u32 (4 bytes, big-endian) - identifies which window the event is for
//! - Event-specific data: variable length based on event type
//!
//! Event Types:
//! - 0x01: Key press
//! - 0x02: Key release
//! - 0x03: Mouse move (relative coordinates)
//! - 0x04: Mouse button press
//! - 0x05: Mouse button release
//! - 0x06: Mouse scroll/wheel

use bytes::{Buf, BufMut, BytesMut};

/// Size of the fixed event header (event_type + window_id)
pub const EVENT_HEADER_SIZE: usize = 5; // 1 byte type + 4 bytes window_id

/// Input event types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum InputEventType {
    /// Key press event
    KeyPress = 0x01,
    /// Key release event
    KeyRelease = 0x02,
    /// Mouse move event (relative to window)
    MouseMove = 0x03,
    /// Mouse button press
    MouseButtonPress = 0x04,
    /// Mouse button release
    MouseButtonRelease = 0x05,
    /// Mouse scroll/wheel event
    MouseScroll = 0x06,
}

impl InputEventType {
    /// Convert u8 to InputEventType
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0x01 => Some(Self::KeyPress),
            0x02 => Some(Self::KeyRelease),
            0x03 => Some(Self::MouseMove),
            0x04 => Some(Self::MouseButtonPress),
            0x05 => Some(Self::MouseButtonRelease),
            0x06 => Some(Self::MouseScroll),
            _ => None,
        }
    }
}

/// Keyboard event data
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    /// Key code (platform-independent, using Linux input event codes)
    pub key_code: u32,
    /// Modifiers bitfield (shift, ctrl, alt, etc.)
    pub modifiers: u8,
}

impl KeyEvent {
    /// Size of encoded KeyEvent in bytes
    pub const ENCODED_SIZE: usize = 5; // 4 bytes key_code + 1 byte modifiers

    /// Encode key event into buffer
    pub fn encode(&self, buf: &mut BytesMut) {
        buf.put_u32(self.key_code);
        buf.put_u8(self.modifiers);
    }

    /// Decode key event from buffer
    pub fn decode(buf: &mut BytesMut) -> Option<Self> {
        if buf.len() < Self::ENCODED_SIZE {
            return None;
        }
        Some(Self {
            key_code: buf.get_u32(),
            modifiers: buf.get_u8(),
        })
    }
}

/// Mouse move event data
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseMoveEvent {
    /// X coordinate relative to window (can be negative for edge cases)
    pub x: i32,
    /// Y coordinate relative to window (can be negative for edge cases)
    pub y: i32,
}

impl MouseMoveEvent {
    /// Size of encoded MouseMoveEvent in bytes
    pub const ENCODED_SIZE: usize = 8; // 4 bytes x + 4 bytes y

    /// Encode mouse move event into buffer
    pub fn encode(&self, buf: &mut BytesMut) {
        buf.put_i32(self.x);
        buf.put_i32(self.y);
    }

    /// Decode mouse move event from buffer
    pub fn decode(buf: &mut BytesMut) -> Option<Self> {
        if buf.len() < Self::ENCODED_SIZE {
            return None;
        }
        Some(Self {
            x: buf.get_i32(),
            y: buf.get_i32(),
        })
    }
}

/// Mouse button event data
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseButtonEvent {
    /// Button number: 1 = left, 2 = right, 3 = middle, 4+ = additional buttons
    pub button: u8,
}

impl MouseButtonEvent {
    /// Size of encoded MouseButtonEvent in bytes
    pub const ENCODED_SIZE: usize = 1; // 1 byte button

    /// Encode mouse button event into buffer
    pub fn encode(&self, buf: &mut BytesMut) {
        buf.put_u8(self.button);
    }

    /// Decode mouse button event from buffer
    pub fn decode(buf: &mut BytesMut) -> Option<Self> {
        if buf.is_empty() {
            return None;
        }
        Some(Self {
            button: buf.get_u8(),
        })
    }
}

/// Mouse scroll/wheel event data
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseScrollEvent {
    /// Horizontal scroll amount (positive = right, negative = left)
    pub horizontal: i32,
    /// Vertical scroll amount (positive = down, negative = up)
    pub vertical: i32,
}

impl MouseScrollEvent {
    /// Size of encoded MouseScrollEvent in bytes
    pub const ENCODED_SIZE: usize = 8; // 4 bytes horizontal + 4 bytes vertical

    /// Encode mouse scroll event into buffer
    pub fn encode(&self, buf: &mut BytesMut) {
        buf.put_i32(self.horizontal);
        buf.put_i32(self.vertical);
    }

    /// Decode mouse scroll event from buffer
    pub fn decode(buf: &mut BytesMut) -> Option<Self> {
        if buf.len() < Self::ENCODED_SIZE {
            return None;
        }
        Some(Self {
            horizontal: buf.get_i32(),
            vertical: buf.get_i32(),
        })
    }
}

/// Input event for forwarding from viewer to server
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    /// Key press
    KeyPress(KeyEvent),
    /// Key release
    KeyRelease(KeyEvent),
    /// Mouse move
    MouseMove(MouseMoveEvent),
    /// Mouse button press
    MouseButtonPress(MouseButtonEvent),
    /// Mouse button release
    MouseButtonRelease(MouseButtonEvent),
    /// Mouse scroll
    MouseScroll(MouseScrollEvent),
}

/// Complete input event with window ID
#[derive(Debug, Clone, PartialEq)]
pub struct WindowInputEvent {
    /// Target window ID
    pub window_id: u32,
    /// The input event
    pub event: InputEvent,
}

impl WindowInputEvent {
    /// Get the event type for this event
    fn event_type(&self) -> InputEventType {
        match &self.event {
            InputEvent::KeyPress(_) => InputEventType::KeyPress,
            InputEvent::KeyRelease(_) => InputEventType::KeyRelease,
            InputEvent::MouseMove(_) => InputEventType::MouseMove,
            InputEvent::MouseButtonPress(_) => InputEventType::MouseButtonPress,
            InputEvent::MouseButtonRelease(_) => InputEventType::MouseButtonRelease,
            InputEvent::MouseScroll(_) => InputEventType::MouseScroll,
        }
    }

    /// Encode the complete event into a buffer
    ///
    /// Format:
    /// - event_type: u8 (1 byte)
    /// - window_id: u32 (4 bytes, big-endian)
    /// - event_data: variable length based on event type
    pub fn encode(&self) -> BytesMut {
        let event_data_size = match &self.event {
            InputEvent::KeyPress(k) | InputEvent::KeyRelease(k) => KeyEvent::ENCODED_SIZE,
            InputEvent::MouseMove(_) => MouseMoveEvent::ENCODED_SIZE,
            InputEvent::MouseButtonPress(_) | InputEvent::MouseButtonRelease(_) => {
                MouseButtonEvent::ENCODED_SIZE
            }
            InputEvent::MouseScroll(_) => MouseScrollEvent::ENCODED_SIZE,
        };

        let mut buf = BytesMut::with_capacity(EVENT_HEADER_SIZE + event_data_size);
        buf.put_u8(self.event_type() as u8);
        buf.put_u32(self.window_id);

        match &self.event {
            InputEvent::KeyPress(k) | InputEvent::KeyRelease(k) => k.encode(&mut buf),
            InputEvent::MouseMove(m) => m.encode(&mut buf),
            InputEvent::MouseButtonPress(b) | InputEvent::MouseButtonRelease(b) => b.encode(&mut buf),
            InputEvent::MouseScroll(s) => s.encode(&mut buf),
        }

        buf
    }

    /// Decode an event from a buffer
    ///
    /// Returns None if the buffer doesn't contain a complete event
    pub fn decode(buf: &[u8]) -> Option<(Self, usize)> {
        if buf.len() < EVENT_HEADER_SIZE {
            return None;
        }

        let event_type = InputEventType::from_u8(buf[0])?;
        let window_id = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]);

        let event_data = &buf[EVENT_HEADER_SIZE..];
        let mut event_buf = BytesMut::from(event_data);

        let (event, event_size) = match event_type {
            InputEventType::KeyPress => {
                let key = KeyEvent::decode(&mut event_buf)?;
                (InputEvent::KeyPress(key), KeyEvent::ENCODED_SIZE)
            }
            InputEventType::KeyRelease => {
                let key = KeyEvent::decode(&mut event_buf)?;
                (InputEvent::KeyRelease(key), KeyEvent::ENCODED_SIZE)
            }
            InputEventType::MouseMove => {
                let mouse = MouseMoveEvent::decode(&mut event_buf)?;
                (InputEvent::MouseMove(mouse), MouseMoveEvent::ENCODED_SIZE)
            }
            InputEventType::MouseButtonPress => {
                let btn = MouseButtonEvent::decode(&mut event_buf)?;
                (InputEvent::MouseButtonPress(btn), MouseButtonEvent::ENCODED_SIZE)
            }
            InputEventType::MouseButtonRelease => {
                let btn = MouseButtonEvent::decode(&mut event_buf)?;
                (InputEvent::MouseButtonRelease(btn), MouseButtonEvent::ENCODED_SIZE)
            }
            InputEventType::MouseScroll => {
                let scroll = MouseScrollEvent::decode(&mut event_buf)?;
                (InputEvent::MouseScroll(scroll), MouseScrollEvent::ENCODED_SIZE)
            }
        };

        let total_size = EVENT_HEADER_SIZE + event_size;
        if buf.len() < total_size {
            return None;
        }

        Some((
            Self {
                window_id,
                event,
            },
            total_size,
        ))
    }
}

/// Input event handler trait
///
/// Implement this trait to receive and process input events
pub trait InputEventHandler: Send + Sync {
    /// Handle an input event
    ///
    /// # Arguments
    /// * `window_id` - The target window ID
    /// * `event` - The input event to process
    fn handle_input_event(&mut self, window_id: u32, event: InputEvent);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_type_from_u8() {
        assert_eq!(InputEventType::from_u8(0x01), Some(InputEventType::KeyPress));
        assert_eq!(InputEventType::from_u8(0x02), Some(InputEventType::KeyRelease));
        assert_eq!(InputEventType::from_u8(0x03), Some(InputEventType::MouseMove));
        assert_eq!(InputEventType::from_u8(0x04), Some(InputEventType::MouseButtonPress));
        assert_eq!(InputEventType::from_u8(0x05), Some(InputEventType::MouseButtonRelease));
        assert_eq!(InputEventType::from_u8(0x06), Some(InputEventType::MouseScroll));
        assert_eq!(InputEventType::from_u8(0xFF), None);
        assert_eq!(InputEventType::from_u8(0x00), None);
    }

    #[test]
    fn test_key_event_encode_decode() {
        let event = KeyEvent {
            key_code: 0x12345678,
            modifiers: 0xAB,
        };
        let mut buf = BytesMut::with_capacity(KeyEvent::ENCODED_SIZE);
        event.encode(&mut buf);

        let mut decode_buf = buf.clone();
        let decoded = KeyEvent::decode(&mut decode_buf).unwrap();
        assert_eq!(event, decoded);
    }

    #[test]
    fn test_key_event_decode_insufficient_data() {
        let buf = BytesMut::from(&[0x01, 0x02, 0x03][..]);
        let mut decode_buf = buf.clone();
        assert!(KeyEvent::decode(&mut decode_buf).is_none());
    }

    #[test]
    fn test_mouse_move_event_encode_decode() {
        let event = MouseMoveEvent { x: -100, y: 200 };
        let mut buf = BytesMut::with_capacity(MouseMoveEvent::ENCODED_SIZE);
        event.encode(&mut buf);

        let mut decode_buf = buf.clone();
        let decoded = MouseMoveEvent::decode(&mut decode_buf).unwrap();
        assert_eq!(event, decoded);
    }

    #[test]
    fn test_mouse_move_event_negative_coordinates() {
        let event = MouseMoveEvent { x: -500, y: -300 };
        let mut buf = BytesMut::with_capacity(MouseMoveEvent::ENCODED_SIZE);
        event.encode(&mut buf);

        let mut decode_buf = buf.clone();
        let decoded = MouseMoveEvent::decode(&mut decode_buf).unwrap();
        assert_eq!(event.x, decoded.x);
        assert_eq!(event.y, decoded.y);
    }

    #[test]
    fn test_mouse_button_event_encode_decode() {
        let event = MouseButtonEvent { button: 2 };
        let mut buf = BytesMut::with_capacity(MouseButtonEvent::ENCODED_SIZE);
        event.encode(&mut buf);

        let mut decode_buf = buf.clone();
        let decoded = MouseButtonEvent::decode(&mut decode_buf).unwrap();
        assert_eq!(event, decoded);
    }

    #[test]
    fn test_mouse_scroll_event_encode_decode() {
        let event = MouseScrollEvent {
            horizontal: -3,
            vertical: 5,
        };
        let mut buf = BytesMut::with_capacity(MouseScrollEvent::ENCODED_SIZE);
        event.encode(&mut buf);

        let mut decode_buf = buf.clone();
        let decoded = MouseScrollEvent::decode(&mut decode_buf).unwrap();
        assert_eq!(event, decoded);
    }

    #[test]
    fn test_window_input_event_key_press_encode_decode() {
        let event = WindowInputEvent {
            window_id: 42,
            event: InputEvent::KeyPress(KeyEvent {
                key_code: 0x1E, // 'a' key in Linux input codes
                modifiers: 0x01, // Shift
            }),
        };

        let encoded = event.encode();
        let (decoded, size) = WindowInputEvent::decode(&encoded).unwrap();

        assert_eq!(event.window_id, decoded.window_id);
        assert_eq!(event.event, decoded.event);
        assert_eq!(size, EVENT_HEADER_SIZE + KeyEvent::ENCODED_SIZE);
    }

    #[test]
    fn test_window_input_event_mouse_move_encode_decode() {
        let event = WindowInputEvent {
            window_id: 100,
            event: InputEvent::MouseMove(MouseMoveEvent { x: 500, y: 300 }),
        };

        let encoded = event.encode();
        let (decoded, size) = WindowInputEvent::decode(&encoded).unwrap();

        assert_eq!(event.window_id, decoded.window_id);
        assert_eq!(event.event, decoded.event);
        assert_eq!(size, EVENT_HEADER_SIZE + MouseMoveEvent::ENCODED_SIZE);
    }

    #[test]
    fn test_window_input_event_mouse_button_encode_decode() {
        let event = WindowInputEvent {
            window_id: 1,
            event: InputEvent::MouseButtonPress(MouseButtonEvent { button: 1 }),
        };

        let encoded = event.encode();
        let (decoded, size) = WindowInputEvent::decode(&encoded).unwrap();

        assert_eq!(event.window_id, decoded.window_id);
        assert_eq!(event.event, decoded.event);
        assert_eq!(size, EVENT_HEADER_SIZE + MouseButtonEvent::ENCODED_SIZE);
    }

    #[test]
    fn test_window_input_event_mouse_scroll_encode_decode() {
        let event = WindowInputEvent {
            window_id: 5,
            event: InputEvent::MouseScroll(MouseScrollEvent {
                horizontal: 0,
                vertical: -3,
            }),
        };

        let encoded = event.encode();
        let (decoded, size) = WindowInputEvent::decode(&encoded).unwrap();

        assert_eq!(event.window_id, decoded.window_id);
        assert_eq!(event.event, decoded.event);
        assert_eq!(size, EVENT_HEADER_SIZE + MouseScrollEvent::ENCODED_SIZE);
    }

    #[test]
    fn test_decode_insufficient_header() {
        let buf = vec![0x01, 0x00, 0x00]; // Only 3 bytes, need 5
        assert!(WindowInputEvent::decode(&buf).is_none());
    }

    #[test]
    fn test_decode_insufficient_event_data() {
        // Key press event header with no key data
        let buf = vec![0x01, 0x00, 0x00, 0x00, 0x01]; // Header only, missing key data
        assert!(WindowInputEvent::decode(&buf).is_none());
    }

    #[test]
    fn test_decode_invalid_event_type() {
        let buf = vec![0xFF, 0x00, 0x00, 0x00, 0x01]; // Invalid event type 0xFF
        assert!(WindowInputEvent::decode(&buf).is_none());
    }

    #[test]
    fn test_all_event_types_roundtrip() {
        let events = vec![
            WindowInputEvent {
                window_id: 1,
                event: InputEvent::KeyPress(KeyEvent {
                    key_code: 30,
                    modifiers: 0,
                }),
            },
            WindowInputEvent {
                window_id: 1,
                event: InputEvent::KeyRelease(KeyEvent {
                    key_code: 30,
                    modifiers: 0,
                }),
            },
            WindowInputEvent {
                window_id: 2,
                event: InputEvent::MouseMove(MouseMoveEvent { x: 100, y: 200 }),
            },
            WindowInputEvent {
                window_id: 2,
                event: InputEvent::MouseButtonPress(MouseButtonEvent { button: 1 }),
            },
            WindowInputEvent {
                window_id: 2,
                event: InputEvent::MouseButtonRelease(MouseButtonEvent { button: 1 }),
            },
            WindowInputEvent {
                window_id: 3,
                event: InputEvent::MouseScroll(MouseScrollEvent {
                    horizontal: 0,
                    vertical: 1,
                }),
            },
        ];

        for event in events {
            let encoded = event.encode();
            let (decoded, _) = WindowInputEvent::decode(&encoded).unwrap();
            assert_eq!(event, decoded, "Event roundtrip failed");
        }
    }

    #[test]
    fn test_big_endian_encoding() {
        let event = WindowInputEvent {
            window_id: 0x12345678,
            event: InputEvent::MouseMove(MouseMoveEvent { x: 0, y: 0 }),
        };

        let encoded = event.encode();

        // Verify window_id is big-endian
        assert_eq!(encoded[1], 0x12);
        assert_eq!(encoded[2], 0x34);
        assert_eq!(encoded[3], 0x56);
        assert_eq!(encoded[4], 0x78);
    }
}

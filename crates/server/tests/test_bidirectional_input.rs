//! Bidirectional Input Tests (S08)
//!
//! These tests verify that input events are correctly encoded, transmitted,
//! and processed between the Windows viewer and Linux compositor.

use std::collections::HashMap;
use wayland_remote_server::streaming::input::InputEventHandler;

/// Test that the input event types exist
///
/// Verifies that all input event types are available.
#[test]
fn test_input_event_types_available() {
    // Verify event type enum exists
    use wayland_remote_server::streaming::input::InputEventType;
    
    let type_name = std::any::type_name::<InputEventType>();
    assert!(type_name.contains("InputEventType"));
    
    // Verify all event types exist
    assert_eq!(InputEventType::KeyPress as u8, 0x01);
    assert_eq!(InputEventType::KeyRelease as u8, 0x02);
    assert_eq!(InputEventType::MouseMove as u8, 0x03);
    assert_eq!(InputEventType::MouseButtonPress as u8, 0x04);
    assert_eq!(InputEventType::MouseButtonRelease as u8, 0x05);
    assert_eq!(InputEventType::MouseScroll as u8, 0x06);
}

/// Test event type conversion
#[test]
fn test_event_type_from_u8() {
    use wayland_remote_server::streaming::input::InputEventType;
    
    assert_eq!(InputEventType::from_u8(0x01), Some(InputEventType::KeyPress));
    assert_eq!(InputEventType::from_u8(0x02), Some(InputEventType::KeyRelease));
    assert_eq!(InputEventType::from_u8(0x03), Some(InputEventType::MouseMove));
    assert_eq!(InputEventType::from_u8(0x04), Some(InputEventType::MouseButtonPress));
    assert_eq!(InputEventType::from_u8(0x05), Some(InputEventType::MouseButtonRelease));
    assert_eq!(InputEventType::from_u8(0x06), Some(InputEventType::MouseScroll));
    assert_eq!(InputEventType::from_u8(0xFF), None);
    assert_eq!(InputEventType::from_u8(0x00), None);
}

/// Test key event structure
#[test]
fn test_key_event_structure() {
    use wayland_remote_server::streaming::input::KeyEvent;
    
    let event = KeyEvent {
        key_code: 30, // 'a' key
        modifiers: 0x01, // Shift
    };
    
    assert_eq!(event.key_code, 30);
    assert_eq!(event.modifiers, 0x01);
    assert_eq!(KeyEvent::ENCODED_SIZE, 5);
}

/// Test key event encode/decode
#[test]
fn test_key_event_encode_decode() {
    use wayland_remote_server::streaming::input::KeyEvent;
    use bytes::BytesMut;
    
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

/// Test mouse move event structure
#[test]
fn test_mouse_move_event_structure() {
    use wayland_remote_server::streaming::input::MouseMoveEvent;
    
    let event = MouseMoveEvent { x: 100, y: 200 };
    assert_eq!(event.x, 100);
    assert_eq!(event.y, 200);
    assert_eq!(MouseMoveEvent::ENCODED_SIZE, 8);
}

/// Test mouse move event encode/decode with negative coordinates
#[test]
fn test_mouse_move_event_negative_coordinates() {
    use wayland_remote_server::streaming::input::MouseMoveEvent;
    use bytes::BytesMut;
    
    let event = MouseMoveEvent { x: -500, y: -300 };
    let mut buf = BytesMut::with_capacity(MouseMoveEvent::ENCODED_SIZE);
    event.encode(&mut buf);
    
    let mut decode_buf = buf.clone();
    let decoded = MouseMoveEvent::decode(&mut decode_buf).unwrap();
    assert_eq!(event.x, decoded.x);
    assert_eq!(event.y, decoded.y);
}

/// Test mouse button event structure
#[test]
fn test_mouse_button_event_structure() {
    use wayland_remote_server::streaming::input::MouseButtonEvent;
    
    let event = MouseButtonEvent { button: 1 };
    assert_eq!(event.button, 1);
    assert_eq!(MouseButtonEvent::ENCODED_SIZE, 1);
}

/// Test mouse button event encode/decode
#[test]
fn test_mouse_button_event_encode_decode() {
    use wayland_remote_server::streaming::input::MouseButtonEvent;
    use bytes::BytesMut;
    
    let event = MouseButtonEvent { button: 3 };
    let mut buf = BytesMut::with_capacity(MouseButtonEvent::ENCODED_SIZE);
    event.encode(&mut buf);
    
    let mut decode_buf = buf.clone();
    let decoded = MouseButtonEvent::decode(&mut decode_buf).unwrap();
    assert_eq!(event, decoded);
}

/// Test mouse scroll event structure
#[test]
fn test_mouse_scroll_event_structure() {
    use wayland_remote_server::streaming::input::MouseScrollEvent;
    
    let event = MouseScrollEvent {
        horizontal: -3,
        vertical: 5,
    };
    assert_eq!(event.horizontal, -3);
    assert_eq!(event.vertical, 5);
    assert_eq!(MouseScrollEvent::ENCODED_SIZE, 8);
}

/// Test mouse scroll event encode/decode
#[test]
fn test_mouse_scroll_event_encode_decode() {
    use wayland_remote_server::streaming::input::MouseScrollEvent;
    use bytes::BytesMut;
    
    let event = MouseScrollEvent {
        horizontal: -10,
        vertical: 20,
    };
    let mut buf = BytesMut::with_capacity(MouseScrollEvent::ENCODED_SIZE);
    event.encode(&mut buf);
    
    let mut decode_buf = buf.clone();
    let decoded = MouseScrollEvent::decode(&mut decode_buf).unwrap();
    assert_eq!(event, decoded);
}

/// Test complete window input event (key press)
#[test]
fn test_window_input_event_key_press() {
    use wayland_remote_server::streaming::input::{
        WindowInputEvent, InputEvent, KeyEvent, EVENT_HEADER_SIZE
    };
    
    let event = WindowInputEvent {
        window_id: 42,
        event: InputEvent::KeyPress(KeyEvent {
            key_code: 30, // 'a' key
            modifiers: 0x01, // Shift
        }),
    };
    
    let encoded = event.encode();
    let (decoded, size) = WindowInputEvent::decode(&encoded).unwrap();
    
    assert_eq!(event.window_id, decoded.window_id);
    assert_eq!(event.event, decoded.event);
    assert_eq!(size, EVENT_HEADER_SIZE + 5); // header + key event
}

/// Test complete window input event (mouse move)
#[test]
fn test_window_input_event_mouse_move() {
    use wayland_remote_server::streaming::input::{
        WindowInputEvent, InputEvent, MouseMoveEvent, EVENT_HEADER_SIZE
    };
    
    let event = WindowInputEvent {
        window_id: 100,
        event: InputEvent::MouseMove(MouseMoveEvent { x: 500, y: 300 }),
    };
    
    let encoded = event.encode();
    let (decoded, size) = WindowInputEvent::decode(&encoded).unwrap();
    
    assert_eq!(event.window_id, decoded.window_id);
    assert_eq!(event.event, decoded.event);
    assert_eq!(size, EVENT_HEADER_SIZE + 8); // header + mouse move
}

/// Test complete window input event (mouse button)
#[test]
fn test_window_input_event_mouse_button() {
    use wayland_remote_server::streaming::input::{
        WindowInputEvent, InputEvent, MouseButtonEvent, EVENT_HEADER_SIZE
    };
    
    let press = WindowInputEvent {
        window_id: 1,
        event: InputEvent::MouseButtonPress(MouseButtonEvent { button: 1 }),
    };
    
    let encoded = press.encode();
    let (decoded, size) = WindowInputEvent::decode(&encoded).unwrap();
    
    assert_eq!(press.window_id, decoded.window_id);
    assert_eq!(press.event, decoded.event);
    assert_eq!(size, EVENT_HEADER_SIZE + 1);
    
    // Test release
    let release = WindowInputEvent {
        window_id: 1,
        event: InputEvent::MouseButtonRelease(MouseButtonEvent { button: 2 }),
    };
    
    let encoded = release.encode();
    let (decoded, _) = WindowInputEvent::decode(&encoded).unwrap();
    
    assert_eq!(release.event, decoded.event);
}

/// Test complete window input event (mouse scroll)
#[test]
fn test_window_input_event_mouse_scroll() {
    use wayland_remote_server::streaming::input::{
        WindowInputEvent, InputEvent, MouseScrollEvent, EVENT_HEADER_SIZE
    };
    
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
    assert_eq!(size, EVENT_HEADER_SIZE + 8);
}

/// Test all event types roundtrip
#[test]
fn test_all_event_types_roundtrip() {
    use wayland_remote_server::streaming::input::{
        WindowInputEvent, InputEvent, KeyEvent, MouseMoveEvent, MouseButtonEvent, MouseScrollEvent
    };
    
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

/// Test decode with insufficient header
#[test]
fn test_decode_insufficient_header() {
    use wayland_remote_server::streaming::input::WindowInputEvent;
    
    let buf = vec![0x01, 0x00, 0x00]; // Only 3 bytes, need 5
    assert!(WindowInputEvent::decode(&buf).is_none());
}

/// Test decode with invalid event type
#[test]
fn test_decode_invalid_event_type() {
    use wayland_remote_server::streaming::input::WindowInputEvent;
    
    let buf = vec![0xFF, 0x00, 0x00, 0x00, 0x01]; // Invalid event type
    assert!(WindowInputEvent::decode(&buf).is_none());
}

/// Test big-endian encoding
#[test]
fn test_big_endian_encoding() {
    use wayland_remote_server::streaming::input::{
        WindowInputEvent, InputEvent, MouseMoveEvent
    };
    
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

/// Test InputProcessor structure
#[test]
fn test_input_processor_structure() {
    use wayland_remote_server::handlers::input::InputProcessor;
    
    let processor = InputProcessor::new();
    assert_eq!(processor.window_count(), 0);
}

/// Test window registration
#[test]
fn test_input_processor_register_window() {
    use wayland_remote_server::handlers::input::InputProcessor;
    use wayland_server::backend::ObjectId;
    
    let mut processor = InputProcessor::new();
    let surface_id = ObjectId::null();
    
    processor.register_window(1, surface_id.clone());
    assert_eq!(processor.window_count(), 1);
    assert!(processor.has_window(1));
    assert_eq!(processor.get_surface(1), Some(&surface_id));
}

/// Test window unregistration
#[test]
fn test_input_processor_unregister_window() {
    use wayland_remote_server::handlers::input::InputProcessor;
    use wayland_server::backend::ObjectId;
    
    let mut processor = InputProcessor::new();
    let surface_id = ObjectId::null();
    
    processor.register_window(1, surface_id);
    processor.unregister_window(1);
    assert_eq!(processor.window_count(), 0);
    assert!(!processor.has_window(1));
}

/// Test surface unregistration
#[test]
fn test_input_processor_unregister_surface() {
    use wayland_remote_server::handlers::input::InputProcessor;
    use wayland_server::backend::ObjectId;
    
    let mut processor = InputProcessor::new();
    let surface_id = ObjectId::null();
    
    processor.register_window(1, surface_id.clone());
    processor.unregister_surface(&surface_id);
    assert_eq!(processor.window_count(), 0);
}

/// Test reverse window lookup
#[test]
fn test_input_processor_reverse_lookup() {
    use wayland_remote_server::handlers::input::InputProcessor;
    use wayland_server::backend::ObjectId;
    
    let mut processor = InputProcessor::new();
    let surface_id = ObjectId::null();
    
    processor.register_window(42, surface_id.clone());
    assert_eq!(processor.get_window_id(&surface_id), Some(42));
}

/// Test multiple windows
#[test]
fn test_input_processor_multiple_windows() {
    use wayland_remote_server::handlers::input::InputProcessor;
    use wayland_server::backend::ObjectId;
    
    let mut processor = InputProcessor::new();
    
    processor.register_window(1, ObjectId::null());
    processor.register_window(2, ObjectId::null());
    processor.register_window(3, ObjectId::null());
    
    assert_eq!(processor.window_count(), 3);
    
    processor.unregister_window(2);
    assert_eq!(processor.window_count(), 2);
    assert!(!processor.has_window(2));
}

/// Test InputProcessor event handling for unregistered window
#[test]
fn test_input_processor_unregistered_window() {
    use wayland_remote_server::handlers::input::InputProcessor;
    use wayland_remote_server::streaming::input::{InputEvent, KeyEvent};
    
    let mut processor = InputProcessor::new();
    
    // This should not panic, just log a warning
    let event = InputEvent::KeyPress(KeyEvent {
        key_code: 30,
        modifiers: 0,
    });
    processor.handle_input_event(999, event);
    
    // Test passes if we reach here
}

/// Test keycode mappings
#[test]
fn test_keycode_mappings() {
    use wayland_remote_server::handlers::input::keycodes::*;
    
    // Test some common mappings
    assert_eq!(vk_to_linux(0x41), Some(KEY_A)); // 'A'
    assert_eq!(vk_to_linux(0x20), Some(KEY_SPACE));
    assert_eq!(vk_to_linux(0x0D), Some(KEY_ENTER));
    assert_eq!(vk_to_linux(0x1B), Some(KEY_ESC));
    assert_eq!(vk_to_linux(0x26), Some(KEY_UP));
    assert_eq!(vk_to_linux(0x70), Some(KEY_F1));
    assert_eq!(vk_to_linux(0x79), Some(KEY_F10));
    
    // Unknown key code returns None
    assert_eq!(vk_to_linux(0xFFFF), None);
}

/// Test modifier flags
#[test]
fn test_modifier_flags() {
    use wayland_remote_server::handlers::input::keycodes::modifiers::*;
    
    assert_eq!(SHIFT, 0x01);
    assert_eq!(CTRL, 0x02);
    assert_eq!(ALT, 0x04);
    assert_eq!(SUPER, 0x08);
}

/// Test InputEventHandler trait implementation
#[test]
fn test_input_event_handler_trait() {
    use wayland_remote_server::handlers::input::InputProcessor;
    use wayland_remote_server::streaming::input::{InputEventHandler, InputEvent, KeyEvent};
    use wayland_server::backend::ObjectId;
    
    let mut processor = InputProcessor::new();
    processor.register_window(1, ObjectId::null());
    
    let event = InputEvent::KeyPress(KeyEvent {
        key_code: 30,
        modifiers: 0x01,
    });
    
    // Call through trait
    InputEventHandler::handle_input_event(&mut processor, 1, event);
    
    // Test passes if we reach here
}

/// Test default implementation of InputProcessor
#[test]
fn test_input_processor_default() {
    use wayland_remote_server::handlers::input::InputProcessor;
    
    let processor: InputProcessor = Default::default();
    assert_eq!(processor.window_count(), 0);
}

/// Integration test: complete input event flow
///
/// This test verifies the complete flow from encoding to decoding.
#[test]
fn test_complete_input_event_flow() {
    use wayland_remote_server::streaming::input::{
        WindowInputEvent, InputEvent, KeyEvent, MouseMoveEvent, MouseButtonEvent,
        MouseScrollEvent
    };
    use wayland_remote_server::handlers::input::InputProcessor;
    use wayland_server::backend::ObjectId;
    
    // Create event
    let event = WindowInputEvent {
        window_id: 42,
        event: InputEvent::KeyPress(KeyEvent {
            key_code: 30, // 'a'
            modifiers: 0x01, // Shift
        }),
    };
    
    // Encode
    let encoded = event.encode();
    
    // Decode
    let (decoded, _) = WindowInputEvent::decode(&encoded).unwrap();
    
    // Verify
    assert_eq!(decoded.window_id, 42);
    match &decoded.event {
        InputEvent::KeyPress(key) => {
            assert_eq!(key.key_code, 30);
            assert_eq!(key.modifiers, 0x01);
        }
        _ => panic!("Expected KeyPress event"),
    }
    
    // Process through InputProcessor
    let mut processor = InputProcessor::new();
    processor.register_window(42, ObjectId::null());
    processor.handle_input_event(42, decoded.event);
    
    // Test passes if we reach here
}

/// Test multiple event types in sequence
#[test]
fn test_multiple_events_sequence() {
    use wayland_remote_server::streaming::input::{
        WindowInputEvent, InputEvent, KeyEvent, MouseMoveEvent, MouseButtonEvent
    };
    use wayland_remote_server::handlers::input::InputProcessor;
    use wayland_server::backend::ObjectId;
    
    let mut processor = InputProcessor::new();
    processor.register_window(1, ObjectId::null());
    
    // Simulate a sequence: mouse move, button down, button up
    let events = vec![
        WindowInputEvent {
            window_id: 1,
            event: InputEvent::MouseMove(MouseMoveEvent { x: 100, y: 100 }),
        },
        WindowInputEvent {
            window_id: 1,
            event: InputEvent::MouseButtonPress(MouseButtonEvent { button: 1 }),
        },
        WindowInputEvent {
            window_id: 1,
            event: InputEvent::MouseMove(MouseMoveEvent { x: 150, y: 150 }),
        },
        WindowInputEvent {
            window_id: 1,
            event: InputEvent::MouseButtonRelease(MouseButtonEvent { button: 1 }),
        },
    ];
    
    for event in events {
        let encoded = event.encode();
        let (decoded, _) = WindowInputEvent::decode(&encoded).unwrap();
        processor.handle_input_event(decoded.window_id, decoded.event);
    }
    
    // Test passes if we reach here without panic
}

/// Integration test: Multiple windows with different events
///
/// This test would verify that events are routed to the correct window.
/// Deferred to integration test phase.
#[test]
#[ignore = "Requires running server and client connection"]
fn test_multiple_windows_event_routing() {
    println!("This test requires a running server and client connection.");
    println!("Deferred to integration test phase.");
    
    // Full implementation would:
    // 1. Start compositor server
    // 2. Connect viewer client
    // 3. Create multiple windows
    // 4. Send events to each window
    // 5. Verify events are routed correctly
    assert!(true);
}

/// Integration test: Network transmission
///
/// This test would verify that events are transmitted over TCP.
/// Deferred to integration test phase.
#[test]
#[ignore = "Requires actual network connection"]
fn test_network_transmission() {
    println!("This test requires actual network connection.");
    println!("Deferred to integration test phase.");
    
    // Full implementation would:
    // 1. Start server listening
    // 2. Connect client
    // 3. Send encoded events from client
    // 4. Verify server receives and decodes correctly
    assert!(true);
}

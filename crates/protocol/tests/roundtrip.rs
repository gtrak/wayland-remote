//! Integration tests for the protocol wire format: message round-trips,
//! truncation safety, frame-header validation, string length limits,
//! unknown-tag handling, and the lz4 block codec.

use std::io::Cursor;

use wayland_remote_protocol::*;

/// Simple LCG for deterministic pseudo-random test data (no rand dep).
fn lcg_fill(buf: &mut [u8], seed: u32) {
    let mut s = seed;
    for b in buf.iter_mut() {
        s = s.wrapping_mul(1664525).wrapping_add(1013904223);
        *b = (s >> 24) as u8;
    }
}

// @lat: [[tests#Protocol#Message round-trip]]
#[test]
fn message_roundtrip() {
    let messages = vec![
        Message::Hello {
            version: 1,
            client_name: "test-viewer".into(),
        },
        Message::Input {
            window_id: 0,
            event: InputEvent::KeyDown { scancode: 0x1E },
        },
        Message::Input {
            window_id: 42,
            event: InputEvent::PointerMove { x: 12.5, y: -3.25 },
        },
        Message::Input {
            window_id: 7,
            event: InputEvent::PointerButton {
                button: 0x110,
                state: ButtonState::Pressed,
            },
        },
        Message::Input {
            window_id: 7,
            event: InputEvent::Axis { dx: 0.0, dy: 1.0 },
        },
        Message::Welcome {
            version: 1,
            width: 1280,
            height: 720,
        },
        Message::WindowEvent {
            window_id: 1,
            event: WindowEventKind::Created {
                width: 800,
                height: 600,
                title: "App".into(),
            },
        },
        Message::WindowEvent {
            window_id: 1,
            event: WindowEventKind::Destroyed,
        },
        Message::Ping {
            timestamp_ns: 1234567890,
        },
        Message::Pong {
            timestamp_ns: 1234567890,
        },
        Message::SetFocus { window_id: 5 },
        Message::ConfigureWindow {
            window_id: 3,
            width: 800,
            height: 600,
        },
        Message::CloseWindow { window_id: 7 },
    ];
    for msg in &messages {
        let mut buf = Vec::new();
        encode_message(msg, &mut buf).unwrap();
        let mut cursor = Cursor::new(&buf);
        let decoded = decode_message(&mut cursor).unwrap();
        assert_eq!(msg, &decoded, "round-trip mismatch");
        // The cursor should be fully consumed (frame length framing).
        assert_eq!(cursor.position() as usize, buf.len());
    }
}

// @lat: [[tests#Protocol#Truncation safety]]
#[test]
fn truncation_safety() {
    let msg = Message::WindowEvent {
        window_id: 99,
        event: WindowEventKind::Created {
            width: 100,
            height: 100,
            title: "x".into(),
        },
    };
    let mut buf = Vec::new();
    encode_message(&msg, &mut buf).unwrap();
    // For every prefix length, decode must return Err, never panic.
    for len in 0..buf.len() {
        let mut cursor = Cursor::new(&buf[..len]);
        let _ = decode_message(&mut cursor); // must not panic
        // (We don't assert Err specifically — the key invariant is no panic
        // on untrusted input.)
    }
}

// @lat: [[tests#Protocol#Frame header validation]]
#[test]
fn frame_header_validation() {
    let valid = FrameHeader {
        magic: FRAME_MAGIC,
        frame_id: 1,
        window_id: 0,
        width: 1280,
        height: 720,
        stride: 1280 * 4,
        format: FORMAT_BGRA8,
        compression: 0,
        _reserved: 0,
        timestamp_ns: 42,
        compressed_size: 1280 * 720 * 4,
    };

    // A valid header round-trips.
    let mut buf = Vec::new();
    encode_frame_header(&valid, &mut buf).unwrap();
    let mut cursor = Cursor::new(&buf);
    let decoded = decode_frame_header(&mut cursor).unwrap();
    assert_eq!(valid, decoded);

    // Bad magic is rejected.
    let mut bad = valid;
    bad.magic = 0xDEADBEEF;
    let mut b = Vec::new();
    encode_frame_header(&bad, &mut b).unwrap();
    assert!(matches!(
        decode_frame_header(&mut Cursor::new(&b)),
        Err(DecodeError::BadFrameMagic)
    ));

    // Unknown pixel format is rejected.
    let mut bad = valid;
    bad.format = 99;
    let mut b = Vec::new();
    encode_frame_header(&bad, &mut b).unwrap();
    assert!(matches!(
        decode_frame_header(&mut Cursor::new(&b)),
        Err(DecodeError::UnknownFormat(99))
    ));

    // Unknown compression value is rejected.
    let mut bad = valid;
    bad.compression = 5;
    let mut b = Vec::new();
    encode_frame_header(&bad, &mut b).unwrap();
    assert!(matches!(
        decode_frame_header(&mut Cursor::new(&b)),
        Err(DecodeError::UnknownCompression(5))
    ));

    // Invalid stride (stride < width * 4) is rejected.
    let mut bad = valid;
    bad.stride = 1280 * 4 - 1;
    let mut b = Vec::new();
    encode_frame_header(&bad, &mut b).unwrap();
    assert!(matches!(
        decode_frame_header(&mut Cursor::new(&b)),
        Err(DecodeError::InvalidStride)
    ));

    // Frame too large (stride * height > 64 MiB) is rejected.
    let mut bad = valid;
    bad.height = 64 * 1024 * 1024 / (1280 * 4) + 1;
    bad.compressed_size = bad.stride as u64 * bad.height as u64;
    let mut b = Vec::new();
    encode_frame_header(&bad, &mut b).unwrap();
    assert!(matches!(
        decode_frame_header(&mut Cursor::new(&b)),
        Err(DecodeError::FrameTooLarge)
    ));
}

// @lat: [[tests#Protocol#String length limit]]
#[test]
fn string_length_limit() {
    // A Hello with a 20,000-byte client name declares a string length over
    // the 16 KiB cap; the codec rejects it during field decode.
    let msg = Message::Hello {
        version: 1,
        client_name: "a".repeat(20_000),
    };
    let mut buf = Vec::new();
    encode_message(&msg, &mut buf).unwrap();
    let result = decode_message(&mut Cursor::new(&buf));
    assert!(matches!(result, Err(DecodeError::StringTooLarge)));
}

// @lat: [[tests#Protocol#Unknown message tag]]
#[test]
fn unknown_message_tag() {
    // Craft a framed message whose tag (99) matches no Message variant.
    let mut payload = Vec::new();
    encode_varint(99, &mut payload).unwrap();
    let mut buf = Vec::new();
    encode_varint(payload.len() as u64, &mut buf).unwrap();
    buf.extend_from_slice(&payload);
    let result = decode_message(&mut Cursor::new(&buf));
    assert!(matches!(result, Err(DecodeError::UnknownMessageTag(99))));
}

// @lat: [[tests#Protocol#lz4 compression round-trip]]
#[test]
fn lz4_compression_roundtrip() {
    // Empty input.
    let empty: Vec<u8> = vec![];
    let c = compress(&empty);
    let d = decompress(&c, 0).unwrap();
    assert_eq!(d, empty);

    // Small, highly compressible input.
    let small = vec![0xAAu8; 64];
    let c = compress(&small);
    let d = decompress(&c, 64).unwrap();
    assert_eq!(d, small);

    // ~1 MiB of pseudo-random (LCG) data.
    let mut big = vec![0u8; 1024 * 1024];
    lcg_fill(&mut big, 0x12345678);
    let c = compress(&big);
    let d = decompress(&c, big.len()).unwrap();
    assert_eq!(d, big);
}

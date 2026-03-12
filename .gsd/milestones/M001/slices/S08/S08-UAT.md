# S08: Bidirectional Input — UAT

**Milestone:** M001
**Written:** 2025-03-12

## UAT Type

- UAT mode: artifact-driven
- Why this mode is sufficient: The bidirectional input implementation is a protocol and data structure feature. Unit tests verify the encoding/decoding, InputProcessor routing logic, and keycode mappings. Full end-to-end testing requires network integration deferred to future slices.

## Preconditions

- Rust toolchain installed
- Dependencies resolved (`cargo build` succeeds)
- Unit test framework available

## Smoke Test

```bash
# Verify the code compiles
cargo build --package wayland-remote-server

# Run all unit tests
cargo test --package wayland-remote-server --test test_bidirectional_input
```

Expected: All 31 tests pass, 2 ignored (integration tests).

## Test Cases

### 1. Event Protocol Encoding

1. Run `cargo test --package wayland-remote-server test_key_event_encode_decode`
2. **Expected:** Test passes, KeyEvent correctly encoded and decoded

### 2. Event Type Roundtrip

1. Run `cargo test --package wayland-remote-server test_all_event_types_roundtrip`
2. **Expected:** All 6 event types (KeyPress, KeyRelease, MouseMove, MouseButtonPress, MouseButtonRelease, MouseScroll) encode and decode correctly

### 3. Mouse Move with Negative Coordinates

1. Run `cargo test --package wayland-remote-server test_mouse_move_event_negative_coordinates`
2. **Expected:** Negative X/Y coordinates survive encoding/decoding roundtrip

### 4. Big-Endian Encoding

1. Run `cargo test --package wayland-remote-server test_big_endian_encoding`
2. **Expected:** Multi-byte values encoded in big-endian format (network byte order)

### 5. InputProcessor Window Registration

1. Run `cargo test --package wayland-remote-server test_input_processor_register_window`
2. **Expected:** Windows can be registered and looked up by ID

### 6. InputProcessor Multiple Windows

1. Run `cargo test --package wayland-remote-server test_input_processor_multiple_windows`
2. **Expected:** Multiple windows tracked independently, unregistration works correctly

### 7. Keycode Mappings

1. Run `cargo test --package wayland-remote-server test_keycode_mappings`
2. **Expected:** Windows virtual keys correctly mapped to Linux input event codes

### 8. Complete Event Flow

1. Run `cargo test --package wayland-remote-server test_complete_input_event_flow`
2. **Expected:** Event encoded → decoded → processed through InputProcessor without errors

### 9. Multiple Events Sequence

1. Run `cargo test --package wayland-remote-server test_multiple_events_sequence`
2. **Expected:** Sequence of mouse events (move → press → move → release) processes correctly

### 10. Invalid Event Type Handling

1. Run `cargo test --package wayland-remote-server test_decode_invalid_event_type`
2. **Expected:** Invalid event type (0xFF) returns None (graceful rejection)

### 11. Insufficient Data Handling

1. Run `cargo test --package wayland-remote-server test_decode_insufficient_header`
2. **Expected:** Incomplete header (3 bytes instead of 5) returns None

### 12. Modifier Flags

1. Run `cargo test --package wayland-remote-server test_modifier_flags`
2. **Expected:** Modifier constants defined: SHIFT=0x01, CTRL=0x02, ALT=0x04, SUPER=0x08

## Edge Cases

### Invalid Window ID

1. Run `cargo test --package wayland-remote-server test_input_processor_unregistered_window`
2. **Expected:** Events for unregistered windows logged but don't panic

### Window-to-Surface Mapping

1. Run `cargo test --package wayland-remote-server test_input_processor_reverse_lookup`
2. **Expected:** Can look up window ID from surface ObjectId (reverse mapping)

## Failure Signals

- Any test failing in `cargo test --package wayland-remote-server --test test_bidirectional_input`
- Build errors when running `cargo build --package wayland-remote-server`
- Clippy warnings about the input module

## Requirements Proved By This UAT

- REQ-INPUT-001 — Binary input event protocol defined, tested, and working
- REQ-INPUT-002 — InputProcessor correctly registers/unregisters windows and routes events
- REQ-INPUT-003 — Keycode mapping between Windows and Linux implemented for common keys
- REQ-INPUT-004 — All event types (keyboard, mouse) encode/decode correctly
- REQ-INPUT-005 — Protocol uses big-endian (consistent with frame protocol)

## Not Proven By This UAT

- Actual network transmission of input events (TCP channel not implemented)
- Wayland seat input injection (stubbed, logs only)
- Full keycode mapping (only common keys mapped)
- Windows viewer input capture (requires Windows environment)
- Integration with actual Wayland client applications
- Performance under high input rate

## Notes for Tester

- This UAT validates the protocol and data structures only
- The actual input event injection into Wayland surfaces is stubbed and logs only
- Full integration testing requires:
  - TCP input channel implementation on server
  - TCP input sender implementation on viewer
  - Running server and viewer with actual network connection
  - Windows environment for viewer testing
- Keycode mapping is partial - if specific keys are needed, check `keycodes::vk_to_linux` implementation

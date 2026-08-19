# Issue 02 — Protocol crate: wire format, framing, lz4 codec

## Objective

Implement the complete `wayland-remote-protocol` crate: message types, little-endian binary encode/decode, varint length framing, and the lz4 block codec. Pure, I/O-free, exhaustively unit-tested. Both server and viewer consume only this crate for wire concerns.

## Files

| File | Change |
|---|---|
| `crates/protocol/src/lib.rs` | Re-exports; crate docs = wire contract summary |
| `crates/protocol/src/message.rs` | `Message` enum + all payload structs |
| `crates/protocol/src/codec.rs` | `Encode`/`Decode` impls, varint, `DecodeError` |
| `crates/protocol/src/compress.rs` | lz4 block codec + `Compression` enum |
| `crates/protocol/tests/roundtrip.rs` | Round-trip + error-injection tests |

## Wire contract spec

All integers little-endian. Every control-stream message is framed as: `varint(len) || bytes(message)`. Stream messages below.

### Messages (control stream, bidirectional)

```rust
enum Message {
    // client -> server (viewer -> compositor)
    Hello { version: u16, client_name: String },        // first message on control stream
    Input { window_id: u64, event: InputEvent },

    // server -> client
    Welcome { version: u16, width: u32, height: u32 },  // response; mismatched version => connection error
    WindowEvent { window_id: u64, event: WindowEventKind },
    Ping { timestamp_ns: u64 },
    Pong { timestamp_ns: u64 },                         // echoes Ping's timestamp
}

enum InputEvent {
    KeyDown { scancode: u16 },                          // Windows scancode; server adds 8 -> linux keycode
    KeyUp   { scancode: u16 },
    PointerMove  { x: f64, y: f64 },                    // surface-local coords
    PointerButton { button: u32, state: ButtonState },  // button = linux BTN_* code
    Axis { dx: f64, dy: f64 },                          // discrete scroll ticks
}

enum WindowEventKind {
    Created  { width: u32, height: u32, title: String },
    Destroyed,
    Resized  { width: u32, height: u32 },
    Focused,
    Unfocused,
}

enum ButtonState { Pressed, Released }
```

### Frame stream (one unidirectional stream per frame)

Header (fixed, 32 bytes) followed by payload:

```rust
struct FrameHeader {   // fields in order, LE
    magic: u32,        // b"FRME"
    frame_id: u64,     // monotonically increasing per connection
    window_id: u64,
    width: u32,
    height: u32,
    stride: u32,       // bytes per row
    format: u8,        // 0 = BGRA8 (only value in M1)
    compression: u8,   // 0 = None, 1 = Lz4
    _reserved: u32,    // zero
    timestamp_ns: u64, // server render time
    compressed_size: u64, // payload bytes after header; == stride*height when compression == None
}
// payload: stride*height BGRA bytes, lz4-block-compressed when compression == 1
// (lz4 block, not frame format; decompress with known uncompressed size = stride*height)
```

`Compression` enum mirrors the u8 values; decoding an unknown u8 is a `DecodeError::UnknownCompression`.

## Steps

1. Implement `codec.rs`: `varint(u64)` encode/decode (LEB128), `Encode`/`Decode` traits over `&mut BytesMut`-style cursors (`impl io::Write`/`io::Read` is fine — keep it std-only), `DecodeError` with `thiserror`.
2. Implement `message.rs` structs + encode/decode for every variant. Strings: `varint(len) || utf8`; reject len > 16 KiB (`DecodeError::StringTooLarge`).
3. Implement `compress.rs`: `compress(&[u8]) -> Vec<u8>` / `decompress(&[u8], expected_len: usize) -> Result<Vec<u8>>` via `lz4_flex::block`; `Compression::from_u8`. Decompress must verify output length == expected.
4. Frame header encode/decode with all field validation (magic, stride >= width*4, stride*height <= 64 MiB cap → `DecodeError::FrameTooLarge`).
5. Tests (each maps to a leaf section added to `lat.md/tests.md` — see Verification).

## Verification

- Round-trip test for every `Message` variant and `FrameHeader` field combination (proptest-style loops with deterministic seeds are fine; no proptest dep needed).
- Error-injection: truncated input at every byte offset of a valid encoding returns `DecodeError`, never panics (loop over `0..len` slicing).
- lz4: round-trip random-ish data (use a simple LCG, no rand dep), plus empty input, plus max-size 64 MiB refusal.
- Frame decode rejects: bad magic, unknown format, unknown compression, stride < width*4, oversized declared size.
- `cargo clippy -p wayland-remote-protocol -- -D warnings` clean; no `unsafe`.
- `lat.md/tests.md` gains leaf sections for each test above with `// @lat:` refs in the test file; `lat check` green.

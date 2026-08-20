---
lat:
  require-code-mention: true
---
# Tests

Test specifications for wayland-remote, mapping 1:1 to tests in code via `// @lat:` comments.

## Protocol

Protocol wire-format tests covering message round-trips, error injection, the lz4 block codec, and frame-header validation.

### Message round-trip

Every `Message` variant encodes and decodes to an equal value across strings, f64s, and enum arms.

### Truncation safety

Decoding any prefix of a valid `Message` encoding returns an error rather than panicking on untrusted input.

### Frame header validation

`FrameHeader` decode rejects bad magic, unknown format, unknown compression, invalid stride, and oversized frames; valid headers round-trip.

### String length limit

A declared string byte length over 16 KiB is rejected as `StringTooLarge`.

### Unknown message tag

An unknown message tag produces a `DecodeError`, not a panic.

### lz4 compression round-trip

lz4 block compress then decompress reproduces the input for empty, small, and 1 MiB inputs.

## Compositor

Headless Smithay compositor integration tests: each test spawns the server in-process on a unique socket and drives it with a real Wayland test client.

### Client connects and creates surface

A test client connects, binds compositor/shm/seat, commits a 64x64 shm surface, and the server reports a tracked surface count of 1.

### Multiple clients supported

Two concurrent test clients each commit a surface and the server reports a tracked surface count of 2, dropping back to 0 when both disconnect.

### Client disconnect cleans up

After the client commits a surface and disconnects, the server removes it from its tracked-surface state and reports a count of 0.

## Rendering

Offscreen pixman rendering tests: the server renders committed client surfaces into a BGRA framebuffer and the test reads the pixels back over a render-request channel.

### Renders client pattern

A client commits a 64x64 surface filled with a known color; a render read-back matches that color exactly at the surface's layout origin, and the region just outside the surface is background black.

### BGRA byte order

Rendering a known opaque blue pixel yields the in-memory bytes `[B, G, R, A]` (not `[R, G, B, A]`) and a contiguous read-back stride of `width * 4`.

### Handles surface resize

After the client re-commits the same surface at a larger size, the next render read-back reflects the new dimensions: a pixel that was background black now matches the surface pattern, and the region beyond the new size is black.

## Streaming

QUIC streaming integration tests: each test spawns a compositor with the QUIC frame server on a free loopback port and drives it with a real quinn client over the wire protocol.

### Handshake and ping

The QUIC handshake completes and a Ping is echoed as Pong with the original timestamp.

### Frame roundtrip

A client surface commits, the server renders and streams a frame, and the viewer receives matching BGRA pixels.

### Version mismatch rejected

A Hello with a wrong protocol version causes the server to close the connection with an application error.

### Frame coalescing

Frames stream to the viewer with monotonically increasing frame ids.

## Viewer

Viewer client tests: pure-function unit tests for the Win32-to-protocol input translation, plus a loopback QUIC session test against a streaming server.

### Scancode extraction

Scancode and extended-key flag are correctly extracted from Win32 key message lParam.

### Button mapping

Win32 mouse button messages map to Linux BTN_* codes with correct pressed/released state.

### Scroll direction

WM_MOUSEWHEEL delta sign maps to the correct vertical scroll direction.

### Key event construction

Scancodes with the extended flag produce the correct KeyDown/KeyUp events with the 0x100 offset.

### Session handshake

The viewer's QUIC client completes the Hello/Welcome handshake and reports the server's dimensions.

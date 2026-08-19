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

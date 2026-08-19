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

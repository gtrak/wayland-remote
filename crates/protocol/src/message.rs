//! Wire-format message types for the wayland-remote protocol.
//!
//! See `docs/plans/001-m1-streaming-linux/02-protocol.md` for the full wire
//! contract and the decision log in `lat.md/decisions.md` for rationale.

/// A control-stream message exchanged between viewer and compositor.
#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    /// Viewer -> compositor: first message, identifies the client.
    Hello { version: u16, client_name: String },
    /// Viewer -> compositor: an input event to inject into the Wayland seat.
    Input { window_id: u64, event: InputEvent },
    /// Compositor -> viewer: handshake response with the negotiated geometry.
    Welcome {
        version: u16,
        width: u32,
        height: u32,
    },
    /// Compositor -> viewer: a window lifecycle event.
    WindowEvent {
        window_id: u64,
        event: WindowEventKind,
    },
    /// Compositor -> viewer / viewer -> compositor: latency probe.
    Ping { timestamp_ns: u64 },
    /// Echo of a Ping with the original timestamp.
    Pong { timestamp_ns: u64 },
}

/// An input event forwarded from the Windows viewer to the Linux compositor.
#[derive(Debug, Clone, PartialEq)]
pub enum InputEvent {
    KeyDown { scancode: u16 },
    KeyUp { scancode: u16 },
    PointerMove { x: f64, y: f64 },
    PointerButton { button: u32, state: ButtonState },
    Axis { dx: f64, dy: f64 },
}

/// A window lifecycle event sent from compositor to viewer.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowEventKind {
    Created {
        width: u32,
        height: u32,
        title: String,
    },
    Destroyed,
    Resized {
        width: u32,
        height: u32,
    },
    Focused,
    Unfocused,
}

/// State of a pointer button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ButtonState {
    Pressed,
    Released,
}

/// Fixed header at the start of every frame's unidirectional stream.
///
/// All fields are little-endian on the wire. The layout sums to 54 bytes
/// (the plan prose said 32, but the field list is authoritative).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameHeader {
    /// Magic bytes `b"FRME"` read as little-endian u32.
    pub magic: u32,
    /// Monotonically increasing frame counter per connection.
    pub frame_id: u64,
    /// Which window this frame belongs to.
    pub window_id: u64,
    /// Frame width in pixels.
    pub width: u32,
    /// Frame height in pixels.
    pub height: u32,
    /// Bytes per scanline (may exceed `width * 4` due to pixman padding).
    pub stride: u32,
    /// Pixel format; `0` = BGRA8 (the only value in milestone 1).
    pub format: u8,
    /// Compression algorithm; `0` = None, `1` = Lz4.
    pub compression: u8,
    /// Reserved, must be zero.
    pub _reserved: u32,
    /// Server render timestamp in nanoseconds.
    pub timestamp_ns: u64,
    /// Payload byte count after this header. Equals `stride * height` when
    /// `compression` is None; the compressed length otherwise.
    pub compressed_size: u64,
}

/// Magic value for `FrameHeader::magic`: `b"FRME"` as a little-endian u32.
pub const FRAME_MAGIC: u32 = u32::from_le_bytes(*b"FRME");

/// BGRA8 pixel format identifier (`FrameHeader::format == FORMAT_BGRA8`).
pub const FORMAT_BGRA8: u8 = 0;

/// Wire size of the frame header in bytes (sum of LE-encoded fields).
///
/// This is the byte count the codec reads/writes; the in-memory struct size
/// may differ due to alignment and is irrelevant to the wire format.
pub const FRAME_HEADER_SIZE: usize = 54;

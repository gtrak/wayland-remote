//! Binary (de)serialization of the wire format.
//!
//! All integers are little-endian on the wire. Every control-stream message
//! is framed as `varint(len) || bytes(message)`; the payload begins with a
//! varint tag identifying the variant:
//!
//! - `Message`: Hello=1, Input=2, Welcome=3, WindowEvent=4, Ping=5, Pong=6,
//!   SetFocus=7, ConfigureWindow=8, CloseWindow=9
//! - `InputEvent`: KeyDown=1, KeyUp=2, PointerMove=3, PointerButton=4, Axis=5
//! - `WindowEventKind`: Created=1, Destroyed=2, Resized=3, Focused=4,
//!   Unfocused=5
//! - `ButtonState`: Pressed=0, Released=1
//!
//! Strings are `varint(len) || utf8` with a 16 KiB length cap.

use std::io;

use crate::message::*;

/// Errors that can occur while decoding the wire format.
#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
    #[error("io error")]
    Io(#[from] io::Error),
    #[error("string too large (max 16 KiB)")]
    StringTooLarge,
    #[error("frame too large (max 64 MiB)")]
    FrameTooLarge,
    #[error("bad frame magic")]
    BadFrameMagic,
    #[error("unknown format {0}")]
    UnknownFormat(u8),
    #[error("unknown compression {0}")]
    UnknownCompression(u8),
    #[error("invalid stride (must be >= width * 4)")]
    InvalidStride,
    #[error("unexpected end of input")]
    UnexpectedEof,
    #[error("invalid utf8")]
    InvalidUtf8(#[from] std::str::Utf8Error),
    #[error("decompress failed")]
    DecompressFailed,
    #[error("unknown message tag {0}")]
    UnknownMessageTag(u64),
    #[error("unknown input event tag {0}")]
    UnknownInputTag(u64),
    #[error("unknown window event tag {0}")]
    UnknownWindowTag(u64),
}

/// Map an [`io::Error`] to [`DecodeError`], translating `UnexpectedEof` into
/// the dedicated [`DecodeError::UnexpectedEof`] variant.
fn eof_or(e: io::Error) -> DecodeError {
    if e.kind() == io::ErrorKind::UnexpectedEof {
        DecodeError::UnexpectedEof
    } else {
        DecodeError::Io(e)
    }
}

/// Encode `value` as an unsigned LEB128 varint.
pub fn encode_varint(mut value: u64, w: &mut impl io::Write) -> io::Result<()> {
    while value >= 0x80 {
        w.write_all(&[(value as u8) | 0x80])?;
        value >>= 7;
    }
    w.write_all(&[value as u8])
}

/// Decode an unsigned LEB128 varint.
pub fn decode_varint(r: &mut impl io::Read) -> Result<u64, DecodeError> {
    let mut result = 0u64;
    let mut shift = 0u32;
    loop {
        let mut byte = [0u8; 1];
        r.read_exact(&mut byte).map_err(eof_or)?;
        result |= ((byte[0] & 0x7f) as u64) << shift;
        if byte[0] & 0x80 == 0 {
            break;
        }
        shift += 7;
        if shift >= 64 {
            return Err(DecodeError::UnexpectedEof); // varint too long
        }
    }
    Ok(result)
}

/// Write a little-endian `u16`.
fn write_u16(v: u16, w: &mut impl io::Write) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

/// Write a little-endian `u32`.
fn write_u32(v: u32, w: &mut impl io::Write) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

/// Write a little-endian `u64`.
fn write_u64(v: u64, w: &mut impl io::Write) -> io::Result<()> {
    w.write_all(&v.to_le_bytes())
}

/// Write a `u8`.
fn write_u8(v: u8, w: &mut impl io::Write) -> io::Result<()> {
    w.write_all(&[v])
}

/// Write an `f64` as its little-endian IEEE-754 bits.
fn write_f64(v: f64, w: &mut impl io::Write) -> io::Result<()> {
    w.write_all(&v.to_bits().to_le_bytes())
}

/// Read a little-endian `u16`.
fn read_u16(r: &mut impl io::Read) -> Result<u16, DecodeError> {
    let mut b = [0u8; 2];
    r.read_exact(&mut b).map_err(eof_or)?;
    Ok(u16::from_le_bytes(b))
}

/// Read a little-endian `u32`.
fn read_u32(r: &mut impl io::Read) -> Result<u32, DecodeError> {
    let mut b = [0u8; 4];
    r.read_exact(&mut b).map_err(eof_or)?;
    Ok(u32::from_le_bytes(b))
}

/// Read a little-endian `u64`.
fn read_u64(r: &mut impl io::Read) -> Result<u64, DecodeError> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b).map_err(eof_or)?;
    Ok(u64::from_le_bytes(b))
}

/// Read a `u8`.
fn read_u8(r: &mut impl io::Read) -> Result<u8, DecodeError> {
    let mut b = [0u8; 1];
    r.read_exact(&mut b).map_err(eof_or)?;
    Ok(b[0])
}

/// Read an `f64` from its little-endian IEEE-754 bits.
fn read_f64(r: &mut impl io::Read) -> Result<f64, DecodeError> {
    let mut b = [0u8; 8];
    r.read_exact(&mut b).map_err(eof_or)?;
    Ok(f64::from_bits(u64::from_le_bytes(b)))
}

/// Write a string as `varint(len) || utf8 bytes`.
fn write_string(s: &str, w: &mut impl io::Write) -> io::Result<()> {
    let bytes = s.as_bytes();
    encode_varint(bytes.len() as u64, w)?;
    w.write_all(bytes)
}

/// Read a string encoded as `varint(len) || utf8 bytes`, enforcing the
/// 16 KiB length cap.
fn read_string(r: &mut impl io::Read) -> Result<String, DecodeError> {
    let len = decode_varint(r)? as usize;
    if len > 16 * 1024 {
        return Err(DecodeError::StringTooLarge);
    }
    let mut buf = vec![0u8; len];
    r.read_exact(&mut buf).map_err(eof_or)?;
    std::str::from_utf8(&buf)
        .map(String::from)
        .map_err(DecodeError::InvalidUtf8)
}

/// Encode a [`Message`] as `varint(len) || tag || fields`.
///
/// The payload (tag + fields) is first encoded into a temporary buffer so
/// the frame length can be written as a leading varint.
pub fn encode_message(msg: &Message, w: &mut impl io::Write) -> io::Result<()> {
    let mut buf = Vec::new();
    match msg {
        Message::Hello {
            version,
            client_name,
        } => {
            encode_varint(1, &mut buf)?;
            write_u16(*version, &mut buf)?;
            write_string(client_name, &mut buf)?;
        }
        Message::Input { window_id, event } => {
            encode_varint(2, &mut buf)?;
            write_u64(*window_id, &mut buf)?;
            encode_input_event(event, &mut buf)?;
        }
        Message::Welcome {
            version,
            width,
            height,
        } => {
            encode_varint(3, &mut buf)?;
            write_u16(*version, &mut buf)?;
            write_u32(*width, &mut buf)?;
            write_u32(*height, &mut buf)?;
        }
        Message::WindowEvent { window_id, event } => {
            encode_varint(4, &mut buf)?;
            write_u64(*window_id, &mut buf)?;
            encode_window_event_kind(event, &mut buf)?;
        }
        Message::Ping { timestamp_ns } => {
            encode_varint(5, &mut buf)?;
            write_u64(*timestamp_ns, &mut buf)?;
        }
        Message::Pong { timestamp_ns } => {
            encode_varint(6, &mut buf)?;
            write_u64(*timestamp_ns, &mut buf)?;
        }
        Message::SetFocus { window_id } => {
            encode_varint(7, &mut buf)?;
            write_u64(*window_id, &mut buf)?;
        }
        Message::ConfigureWindow {
            window_id,
            width,
            height,
        } => {
            encode_varint(8, &mut buf)?;
            write_u64(*window_id, &mut buf)?;
            write_u32(*width, &mut buf)?;
            write_u32(*height, &mut buf)?;
        }
        Message::CloseWindow { window_id } => {
            encode_varint(9, &mut buf)?;
            write_u64(*window_id, &mut buf)?;
        }
    }
    encode_varint(buf.len() as u64, w)?;
    w.write_all(&buf)
}

/// Decode a [`Message`] framed as `varint(len) || tag || fields`.
///
/// Exactly `len` payload bytes are consumed from the stream and decoded
/// from a cursor over them, so any leftover stream bytes are untouched.
pub fn decode_message(r: &mut impl io::Read) -> Result<Message, DecodeError> {
    let len = decode_varint(r)? as usize;
    let mut payload = vec![0u8; len];
    r.read_exact(&mut payload).map_err(eof_or)?;
    let mut cursor = io::Cursor::new(payload);
    match decode_varint(&mut cursor)? {
        1 => {
            let version = read_u16(&mut cursor)?;
            let client_name = read_string(&mut cursor)?;
            Ok(Message::Hello {
                version,
                client_name,
            })
        }
        2 => {
            let window_id = read_u64(&mut cursor)?;
            let event = decode_input_event(&mut cursor)?;
            Ok(Message::Input { window_id, event })
        }
        3 => {
            let version = read_u16(&mut cursor)?;
            let width = read_u32(&mut cursor)?;
            let height = read_u32(&mut cursor)?;
            Ok(Message::Welcome {
                version,
                width,
                height,
            })
        }
        4 => {
            let window_id = read_u64(&mut cursor)?;
            let event = decode_window_event_kind(&mut cursor)?;
            Ok(Message::WindowEvent { window_id, event })
        }
        5 => Ok(Message::Ping {
            timestamp_ns: read_u64(&mut cursor)?,
        }),
        6 => Ok(Message::Pong {
            timestamp_ns: read_u64(&mut cursor)?,
        }),
        7 => Ok(Message::SetFocus {
            window_id: read_u64(&mut cursor)?,
        }),
        8 => Ok(Message::ConfigureWindow {
            window_id: read_u64(&mut cursor)?,
            width: read_u32(&mut cursor)?,
            height: read_u32(&mut cursor)?,
        }),
        9 => Ok(Message::CloseWindow {
            window_id: read_u64(&mut cursor)?,
        }),
        tag => Err(DecodeError::UnknownMessageTag(tag)),
    }
}

/// Encode an [`InputEvent`] as `tag || fields`.
pub fn encode_input_event(event: &InputEvent, w: &mut impl io::Write) -> io::Result<()> {
    match event {
        InputEvent::KeyDown { scancode } => {
            encode_varint(1, w)?;
            write_u16(*scancode, w)
        }
        InputEvent::KeyUp { scancode } => {
            encode_varint(2, w)?;
            write_u16(*scancode, w)
        }
        InputEvent::PointerMove { x, y } => {
            encode_varint(3, w)?;
            write_f64(*x, w)?;
            write_f64(*y, w)
        }
        InputEvent::PointerButton { button, state } => {
            encode_varint(4, w)?;
            write_u32(*button, w)?;
            write_u8(
                match state {
                    ButtonState::Pressed => 0,
                    ButtonState::Released => 1,
                },
                w,
            )
        }
        InputEvent::Axis { dx, dy } => {
            encode_varint(5, w)?;
            write_f64(*dx, w)?;
            write_f64(*dy, w)
        }
    }
}

/// Decode an [`InputEvent`] encoded as `tag || fields`.
pub fn decode_input_event(r: &mut impl io::Read) -> Result<InputEvent, DecodeError> {
    match decode_varint(r)? {
        1 => Ok(InputEvent::KeyDown {
            scancode: read_u16(r)?,
        }),
        2 => Ok(InputEvent::KeyUp {
            scancode: read_u16(r)?,
        }),
        3 => Ok(InputEvent::PointerMove {
            x: read_f64(r)?,
            y: read_f64(r)?,
        }),
        4 => {
            let button = read_u32(r)?;
            let state = match read_u8(r)? {
                0 => ButtonState::Pressed,
                1 => ButtonState::Released,
                // No dedicated error variant for the button-state sub-tag;
                // report it as an unknown input event tag.
                other => return Err(DecodeError::UnknownInputTag(other as u64)),
            };
            Ok(InputEvent::PointerButton { button, state })
        }
        5 => Ok(InputEvent::Axis {
            dx: read_f64(r)?,
            dy: read_f64(r)?,
        }),
        tag => Err(DecodeError::UnknownInputTag(tag)),
    }
}

/// Encode a [`WindowEventKind`] as `tag || fields`.
pub fn encode_window_event_kind(event: &WindowEventKind, w: &mut impl io::Write) -> io::Result<()> {
    match event {
        WindowEventKind::Created {
            width,
            height,
            title,
        } => {
            encode_varint(1, w)?;
            write_u32(*width, w)?;
            write_u32(*height, w)?;
            write_string(title, w)
        }
        WindowEventKind::Destroyed => encode_varint(2, w),
        WindowEventKind::Resized { width, height } => {
            encode_varint(3, w)?;
            write_u32(*width, w)?;
            write_u32(*height, w)
        }
        WindowEventKind::Focused => encode_varint(4, w),
        WindowEventKind::Unfocused => encode_varint(5, w),
    }
}

/// Decode a [`WindowEventKind`] encoded as `tag || fields`.
pub fn decode_window_event_kind(r: &mut impl io::Read) -> Result<WindowEventKind, DecodeError> {
    match decode_varint(r)? {
        1 => Ok(WindowEventKind::Created {
            width: read_u32(r)?,
            height: read_u32(r)?,
            title: read_string(r)?,
        }),
        2 => Ok(WindowEventKind::Destroyed),
        3 => Ok(WindowEventKind::Resized {
            width: read_u32(r)?,
            height: read_u32(r)?,
        }),
        4 => Ok(WindowEventKind::Focused),
        5 => Ok(WindowEventKind::Unfocused),
        tag => Err(DecodeError::UnknownWindowTag(tag)),
    }
}

/// Encode a [`FrameHeader`] field by field, little-endian.
pub fn encode_frame_header(h: &FrameHeader, w: &mut impl io::Write) -> io::Result<()> {
    write_u32(h.magic, w)?;
    write_u64(h.frame_id, w)?;
    write_u64(h.window_id, w)?;
    write_u32(h.width, w)?;
    write_u32(h.height, w)?;
    write_u32(h.stride, w)?;
    write_u8(h.format, w)?;
    write_u8(h.compression, w)?;
    write_u32(h._reserved, w)?;
    write_u64(h.timestamp_ns, w)?;
    write_u64(h.compressed_size, w)
}

/// Decode and validate a [`FrameHeader`].
///
/// Rejects a bad magic, an unknown pixel format, an unknown compression
/// value, `stride < width * 4`, and a declared frame larger than 64 MiB.
pub fn decode_frame_header(r: &mut impl io::Read) -> Result<FrameHeader, DecodeError> {
    let magic = read_u32(r)?;
    if magic != FRAME_MAGIC {
        return Err(DecodeError::BadFrameMagic);
    }
    let frame_id = read_u64(r)?;
    let window_id = read_u64(r)?;
    let width = read_u32(r)?;
    let height = read_u32(r)?;
    let stride = read_u32(r)?;
    let format = read_u8(r)?;
    if format != FORMAT_BGRA8 {
        return Err(DecodeError::UnknownFormat(format));
    }
    let compression = read_u8(r)?;
    if compression > 1 {
        return Err(DecodeError::UnknownCompression(compression));
    }
    let _reserved = read_u32(r)?;
    let timestamp_ns = read_u64(r)?;
    let compressed_size = read_u64(r)?;
    if (stride as u64) < (width as u64) * 4 {
        return Err(DecodeError::InvalidStride);
    }
    if (stride as u64) * (height as u64) > 64 * 1024 * 1024 {
        return Err(DecodeError::FrameTooLarge);
    }
    Ok(FrameHeader {
        magic,
        frame_id,
        window_id,
        width,
        height,
        stride,
        format,
        compression,
        _reserved,
        timestamp_ns,
        compressed_size,
    })
}

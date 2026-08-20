//! Per-connection QUIC session (plan 001 issue 05).
//!
//! One bidirectional control stream carries the Hello/Welcome handshake,
//! input events, and Ping/Pong. Each frame gets its own unidirectional
//! stream (54-byte [`FrameHeader`] + compressed payload) so a slow or lost
//! frame cannot head-of-line-block newer ones — receivers skip stale
//! streams instead.

use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use calloop::channel;
use quinn::{Connection, RecvStream, SendStream};
use wayland_remote_protocol::{
    Compression, DecodeError, FORMAT_BGRA8, FRAME_HEADER_SIZE, FRAME_MAGIC, FrameHeader, Message,
    WindowEventKind, decode_message, decode_varint, encode_message,
};

use crate::bridge::{CompositorCommand, NetCommand};
use crate::net::{COALESCE_WINDOW, ERROR_VERSION_MISMATCH, PROTOCOL_VERSION, SessionRegistry};
use crate::rendering::FrameBuffer;

/// Maximum size of one framed control message (varint length + payload).
const MAX_MESSAGE_BYTES: usize = 32 * 1024;

/// How long to wait for the client to open its control stream.
const CONTROL_STREAM_TIMEOUT: Duration = Duration::from_secs(10);

/// Interval for server-initiated pings on the control stream.
pub const PING_INTERVAL: Duration = Duration::from_millis(500);

/// Read a [`u64`] nanosecond timestamp (since the UNIX epoch) for Ping.
#[must_use]
pub fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Reads framed control messages from a QUIC stream, preserving any
/// pipelined bytes that spill past a single message.
#[derive(Debug, Default)]
pub struct MessageReader {
    pending: Vec<u8>,
}

impl MessageReader {
    /// Create an empty reader.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: Vec::new(),
        }
    }

    /// Read the next framed message, pulling from `recv` until one is
    /// complete. Returns an error when the stream ends or the message is
    /// malformed/oversized.
    pub async fn next(&mut self, recv: &mut RecvStream) -> anyhow::Result<Message> {
        loop {
            let mut cursor = Cursor::new(&self.pending);
            match decode_varint(&mut cursor) {
                Ok(len) if (len as usize) <= MAX_MESSAGE_BYTES => {
                    let total = cursor.position() as usize + len as usize;
                    if self.pending.len() >= total {
                        let frame: Vec<u8> = self.pending.drain(..total).collect();
                        return decode_message(&mut Cursor::new(frame))
                            .map_err(anyhow::Error::from);
                    }
                }
                Ok(_) => anyhow::bail!("control message exceeds {MAX_MESSAGE_BYTES} bytes"),
                Err(DecodeError::UnexpectedEof) => {
                    // Varint incomplete; need more bytes.
                }
                Err(err) => anyhow::bail!("malformed control message: {err}"),
            }
            let mut buf = [0u8; 8192];
            match recv.read(&mut buf).await? {
                Some(n) => self.pending.extend_from_slice(&buf[..n]),
                None => anyhow::bail!("control stream closed"),
            }
        }
    }
}

/// Write one framed control message to `send`.
pub async fn write_message(send: &mut SendStream, msg: &Message) -> anyhow::Result<()> {
    let mut buf = Vec::new();
    encode_message(msg, &mut buf)?;
    send.write_all(&buf).await?;
    Ok(())
}

/// Handle one viewer connection until it drops.
///
/// `frame_rx` carries (already coalesced) frames from the pump; each frame
/// goes out on a fresh unidirectional stream with the compositor-assigned
/// `frame_id`. The session removes its registry slot on exit.
#[allow(clippy::too_many_arguments)] // one argument per session concern
pub(crate) async fn handle_connection(
    conn: Connection,
    mut frame_rx: tokio::sync::mpsc::UnboundedReceiver<NetCommand>,
    input_tx: channel::SyncSender<CompositorCommand>,
    compression: Compression,
    width: u32,
    height: u32,
    token: Arc<()>,
    sessions: SessionRegistry,
) {
    // Remove the registry slot when the session ends (whatever the cause).
    struct SlotGuard {
        token: Arc<()>,
        sessions: SessionRegistry,
    }
    impl Drop for SlotGuard {
        fn drop(&mut self) {
            self.sessions
                .write()
                .unwrap()
                .retain(|slot| !Arc::ptr_eq(&slot.token, &self.token));
        }
    }
    let _guard = SlotGuard { token, sessions };

    // The client opens the control stream first.
    let (mut ctrl_send, mut ctrl_recv) =
        match tokio::time::timeout(CONTROL_STREAM_TIMEOUT, conn.accept_bi()).await {
            Ok(Ok(streams)) => streams,
            Ok(Err(err)) => {
                tracing::debug!(?err, "session: no control stream");
                return;
            }
            Err(_) => {
                tracing::warn!("session: client never opened a control stream");
                return;
            }
        };

    // Handshake: Hello -> Welcome.
    let mut reader = MessageReader::new();
    let hello = match reader.next(&mut ctrl_recv).await {
        Ok(Message::Hello {
            version,
            client_name,
        }) => (version, client_name),
        Ok(other) => {
            tracing::warn!(?other, "session: first message is not Hello; closing");
            conn.close(0x1000u32.into(), b"expected Hello");
            return;
        }
        Err(err) => {
            tracing::debug!(?err, "session: Hello read failed");
            return;
        }
    };

    if hello.0 != PROTOCOL_VERSION {
        tracing::warn!(
            version = hello.0,
            expected = PROTOCOL_VERSION,
            "session: protocol version mismatch; closing"
        );
        conn.close(ERROR_VERSION_MISMATCH.into(), b"version mismatch");
        return;
    }
    tracing::info!(client = %hello.1, "session: handshake complete");

    if write_message(
        &mut ctrl_send,
        &Message::Welcome {
            version: PROTOCOL_VERSION,
            width,
            height,
        },
    )
    .await
    .is_err()
    {
        tracing::debug!("session: Welcome write failed");
        return;
    }

    // Window events are small and order-sensitive: the frame sender
    // forwards them to the control loop (which owns the control stream)
    // as they arrive, so a coalesced frame burst cannot swallow them.
    let (window_event_tx, mut window_event_rx) =
        tokio::sync::mpsc::unbounded_channel::<Vec<(u64, WindowEventKind)>>();

    // Frame sender: each frame on its own unidirectional stream.
    let mut frame_conn = conn.clone();
    tokio::spawn(async move {
        loop {
            let Some(cmd) = frame_rx.recv().await else {
                break;
            };
            // Sender-side coalescing: absorb anything that landed while the
            // previous frame was on the wire; only the newest frame is
            // written, but window events are forwarded immediately.
            tokio::time::sleep(COALESCE_WINDOW).await;
            let mut batch = vec![cmd];
            while let Ok(cmd) = frame_rx.try_recv() {
                batch.push(cmd);
            }
            let mut latest: Option<NetCommand> = None;
            for cmd in batch {
                match cmd {
                    NetCommand::WindowEvents(events) => {
                        let _ = window_event_tx.send(events);
                    }
                    other => latest = Some(other),
                }
            }
            match latest {
                Some(NetCommand::Frame { frame, frame_id }) => {
                    if write_frame(&mut frame_conn, &frame, frame_id, compression)
                        .await
                        .is_err()
                    {
                        tracing::debug!("session: frame write failed; ending frame stream");
                        break;
                    }
                }
                Some(NetCommand::Shutdown) => break,
                _ => {}
            }
        }
    });

    // Control loop: input events, window commands, window events, ping/pong,
    // and periodic server pings.
    let mut ping_timer = tokio::time::interval(PING_INTERVAL);
    ping_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            msg = reader.next(&mut ctrl_recv) => {
                match msg {
                    Ok(Message::Ping { timestamp_ns }) => {
                        if write_message(&mut ctrl_send, &Message::Pong { timestamp_ns }).await
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(Message::Pong { timestamp_ns }) => {
                        let rtt = now_ns().saturating_sub(timestamp_ns);
                        tracing::debug!(rtt_us = rtt / 1000, "session: pong received");
                    }
                    Ok(Message::Input { event, .. }) => {
                        // Seat input injection is a later milestone; the
                        // event is forwarded to the compositor, which logs it.
                        tracing::debug!(?event, "session: input event");
                        if input_tx.send(CompositorCommand::Input(event)).is_err() {
                            tracing::debug!("session: compositor channel closed");
                            break;
                        }
                    }
                    Ok(Message::SetFocus { window_id }) => {
                        if input_tx
                            .send(CompositorCommand::SetFocus { window_id })
                            .is_err()
                        {
                            tracing::debug!("session: compositor channel closed");
                            break;
                        }
                    }
                    Ok(Message::ConfigureWindow { window_id, width, height }) => {
                        if input_tx
                            .send(CompositorCommand::ConfigureWindow {
                                window_id,
                                width,
                                height,
                            })
                            .is_err()
                        {
                            tracing::debug!("session: compositor channel closed");
                            break;
                        }
                    }
                    Ok(Message::CloseWindow { window_id }) => {
                        if input_tx
                            .send(CompositorCommand::CloseWindow { window_id })
                            .is_err()
                        {
                            tracing::debug!("session: compositor channel closed");
                            break;
                        }
                    }
                    Ok(other) => {
                        tracing::warn!(?other, "session: unexpected control message");
                    }
                    Err(err) => {
                        tracing::debug!(?err, "session: control stream ended");
                        break;
                    }
                }
            }
            events = window_event_rx.recv() => {
                // The frame sender forwards window events; None means it has
                // exited, which ends the session.
                let Some(events) = events else {
                    break;
                };
                for (window_id, event) in events {
                    if write_message(
                        &mut ctrl_send,
                        &Message::WindowEvent { window_id, event },
                    )
                    .await
                    .is_err()
                    {
                        break;
                    }
                }
            }
            _ = ping_timer.tick() => {
                if write_message(&mut ctrl_send, &Message::Ping { timestamp_ns: now_ns() })
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }
    }

    tracing::info!("session: control loop ended; closing connection");
    conn.close(0u32.into(), b"session closed");
}

/// Open a fresh unidirectional stream and write one frame (header + payload).
async fn write_frame(
    conn: &mut Connection,
    frame: &FrameBuffer,
    frame_id: u64,
    compression: Compression,
) -> anyhow::Result<()> {
    let payload = match compression {
        Compression::Lz4 => wayland_remote_protocol::compress(&frame.data),
        Compression::None => frame.data.clone(),
    };
    let header = FrameHeader {
        magic: FRAME_MAGIC,
        frame_id,
        window_id: frame.window_id,
        width: frame.width,
        height: frame.height,
        stride: frame.stride,
        format: FORMAT_BGRA8,
        compression: compression.as_u8(),
        _reserved: 0,
        timestamp_ns: now_ns(),
        compressed_size: payload.len() as u64,
    };
    let mut bytes = Vec::with_capacity(FRAME_HEADER_SIZE + payload.len());
    wayland_remote_protocol::encode_frame_header(&header, &mut bytes)?;
    bytes.extend_from_slice(&payload);

    let mut send = conn.open_uni().await?;
    send.write_all(&bytes).await?;
    send.finish()?;
    Ok(())
}

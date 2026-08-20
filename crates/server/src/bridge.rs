//! calloop ↔ tokio channel bridge (plan 001 issue 05).
//!
//! The compositor runs on a single-threaded calloop event loop that never
//! awaits; all network I/O runs on a tokio runtime. The two sides exchange
//! commands through channels owned here:
//!
//! - Frames and window events out (compositor → network) travel a tokio
//!   unbounded mpsc: the compositor pushes [`NetCommand::Frame`] /
//!   [`NetCommand::WindowEvents`] with a non-blocking `unbounded_send`, and
//!   the network side owns the receiver.
//! - Events in (network → compositor) travel a calloop sync channel: the
//!   network side sends [`CompositorCommand`] values, and the compositor
//!   inserts the channel's receiver into its event loop (the caller's job —
//!   see [`crate::run`]).
//!
//! The compositor thread is never allowed to await (see the Runtime Split
//! decision in lat.md/decisions.md).

use calloop::channel;
use wayland_remote_protocol::{InputEvent, WindowEventKind};

use crate::rendering::{FrameBuffer, RenderRequest};

/// A command from the compositor thread to the network side.
#[derive(Debug, Clone)]
pub enum NetCommand {
    /// A rendered frame to stream to connected viewers.
    Frame {
        /// The rendered frame.
        frame: FrameBuffer,
        /// Compositor-assigned monotonic sequence number; travels as
        /// `FrameHeader::frame_id` so receivers (and tests) can identify
        /// frames coalesced away by the sender.
        frame_id: u64,
    },
    /// Window lifecycle events to relay to every viewer's control stream.
    WindowEvents(Vec<(u64, WindowEventKind)>),
    /// Stop the QUIC server and drop all sessions.
    Shutdown,
}

/// A command from the network side to the compositor thread.
#[derive(Debug)]
pub enum CompositorCommand {
    /// An input event to dispatch into the Wayland seat.
    Input { window_id: u64, event: InputEvent },
    /// A viewer focus request: activate the given window.
    SetFocus { window_id: u64 },
    /// A viewer resize request: configure the given window to a new size.
    ConfigureWindow {
        window_id: u64,
        width: u32,
        height: u32,
    },
    /// A viewer close request: send the xdg close event to the window.
    CloseWindow { window_id: u64 },
    /// A render request handled on the compositor thread.
    RenderRequest(RenderRequest),
}

/// The network side of the bridge: receives frames, sends commands to the
/// compositor.
pub struct NetBridge {
    /// Incoming frames/commands from the compositor (never blocks on send).
    pub frame_rx: tokio::sync::mpsc::UnboundedReceiver<NetCommand>,
    /// Outgoing commands to the compositor (blocks if the event loop is
    /// saturated beyond the channel bound).
    pub input_tx: channel::SyncSender<CompositorCommand>,
}

/// The compositor side of the bridge: sends frames, receives commands.
pub struct CompositorBridge {
    /// Frames/commands pushed to the network side.
    pub frame_tx: tokio::sync::mpsc::UnboundedSender<NetCommand>,
    /// Commands arriving from the network; insert into the event loop.
    pub input_rx: channel::Channel<CompositorCommand>,
}

/// Create the calloop ↔ tokio bridge channel pair.
#[must_use]
pub fn channels() -> (NetBridge, CompositorBridge) {
    let (frame_tx, frame_rx) = tokio::sync::mpsc::unbounded_channel();
    let (input_tx, input_rx) = channel::sync_channel::<CompositorCommand>(100);
    (
        NetBridge { frame_rx, input_tx },
        CompositorBridge { frame_tx, input_rx },
    )
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[test]
    fn frame_round_trips_through_channels() {
        let (mut net, compositor) = channels();

        let frame = FrameBuffer {
            data: vec![1, 2, 3, 4],
            width: 1,
            height: 1,
            stride: 4,
            window_id: 7,
        };
        compositor
            .frame_tx
            .send(NetCommand::Frame {
                frame: frame.clone(),
                frame_id: 7,
            })
            .expect("unbounded send never fails while the receiver is alive");

        // The calloop receiver starts empty (no event loop needed to poll it).
        assert!(
            compositor.input_rx.try_recv().is_err(),
            "no command was sent to the compositor yet"
        );

        // Drive the network-side receiver on a runtime: it must observe the
        // frame the compositor pushed.
        let runtime = tokio::runtime::Runtime::new().expect("runtime");
        let received = runtime.block_on(async move {
            tokio::time::timeout(Duration::from_secs(1), net.frame_rx.recv())
                .await
                .expect("frame should arrive promptly")
        });
        match received {
            Some(NetCommand::Frame { frame, frame_id }) => {
                assert_eq!(frame_id, 7, "sequence number must travel with the frame");
                assert_eq!(frame.data, vec![1u8, 2, 3, 4]);
                assert_eq!((frame.width, frame.height, frame.stride), (1, 1, 4));
                assert_eq!(frame.window_id, 7, "window id must travel with the frame");
            }
            other => panic!("expected a frame, got {other:?}"),
        }
    }
}

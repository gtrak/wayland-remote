//! QUIC frame server (plan 001 issue 05).
//!
//! A quinn endpoint with a self-signed TLS certificate accepts viewer
//! connections. A frame pump forwards compositor frames to every active
//! session, coalescing bursts so only the newest frame in a short window
//! goes on the wire (drop-oldest pacing). Each session owns a control
//! stream plus one unidirectional stream per frame — see
//! [`session`] and the QUIC Session Model in lat.md/architecture.md.

pub mod cert;
pub mod session;

use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use wayland_remote_protocol::Compression;

use crate::bridge::{NetBridge, NetCommand};

use self::cert::ServerCert;

/// Protocol version advertised in the Hello/Welcome handshake.
pub const PROTOCOL_VERSION: u16 = 1;

/// QUIC application error code: Hello/Welcome version mismatch.
pub const ERROR_VERSION_MISMATCH: u32 = 1;

/// Grace window for sender-side frame coalescing: after a frame is popped
/// from the compositor channel, any frames arriving within this window are
/// dropped in favor of the newest.
pub const COALESCE_WINDOW: Duration = Duration::from_millis(2);

/// Settings for the QUIC frame server.
#[derive(Debug)]
pub struct NetSettings {
    /// Address the QUIC endpoint listens on.
    pub listen: SocketAddr,
    /// Frame payload compression (`--raw` selects [`Compression::None`]).
    pub compression: Compression,
    /// Self-signed TLS certificate for the endpoint.
    pub cert: ServerCert,
    /// Output width advertised in Welcome (should match the renderer size).
    pub width: u32,
    /// Output height advertised in Welcome (should match the renderer size).
    pub height: u32,
}

/// One connected viewer session: a registry slot and its frame inlet.
pub(crate) struct SessionSlot {
    /// Identity used to remove the slot when the session ends.
    token: Arc<()>,
    /// Frames delivered to this session.
    tx: tokio::sync::mpsc::UnboundedSender<NetCommand>,
}

/// Active sessions' frame inlets, fanned out to by the frame pump.
pub(crate) type SessionRegistry = Arc<RwLock<Vec<SessionSlot>>>;

/// Forward frames and window events from the compositor channel to every
/// connected session.
///
/// After each received command a short [`COALESCE_WINDOW`] grace period is
/// observed, then the queue is drained: only the newest frame is delivered
/// (sender-side coalescing), while window events are always forwarded in
/// order — they are small and must not be swallowed by a frame burst.
/// A `Shutdown` landing inside the window never swallows a pending frame —
/// the newest frame is delivered first, then the pump exits.
async fn frame_pump(
    mut frame_rx: tokio::sync::mpsc::UnboundedReceiver<NetCommand>,
    sessions: SessionRegistry,
) {
    loop {
        let Some(first) = frame_rx.recv().await else {
            break;
        };
        tokio::time::sleep(COALESCE_WINDOW).await;
        let mut pending = vec![first];
        while let Ok(cmd) = frame_rx.try_recv() {
            pending.push(cmd);
        }
        let mut latest_frame: Option<NetCommand> = None;
        let mut window_events: Vec<NetCommand> = Vec::new();
        let mut shutdown = false;
        for cmd in pending {
            match cmd {
                NetCommand::Frame { .. } => latest_frame = Some(cmd),
                NetCommand::WindowEvents(_) => window_events.push(cmd),
                NetCommand::Shutdown => shutdown = true,
            }
        }
        let txs: Vec<_> = sessions
            .read()
            .unwrap()
            .iter()
            .map(|slot| slot.tx.clone())
            .collect();
        if let Some(NetCommand::Frame { frame, frame_id }) = latest_frame {
            for tx in &txs {
                let _ = tx.send(NetCommand::Frame {
                    frame: frame.clone(),
                    frame_id,
                });
            }
        }
        for events in &window_events {
            for tx in &txs {
                let _ = tx.send(events.clone());
            }
        }
        if shutdown {
            break;
        }
    }
}

/// Run the QUIC frame server on the current tokio runtime.
///
/// Accepts connections until the frame channel delivers
/// `NetCommand::Shutdown` (or the compositor drops its end). The bridge's
/// input channel is cloned into each session for input forwarding.
pub async fn run_server(settings: &NetSettings, net: NetBridge) -> anyhow::Result<()> {
    // Install the aws-lc-rs crypto provider once per process; a second
    // install is a no-op error that we ignore.
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
    let mut rustls_config = rustls::ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_single_cert(
            vec![settings.cert.cert_der.clone()],
            settings.cert.key_der.clone_key(),
        )?;
    rustls_config.alpn_protocols = vec![cert::ALPN_PROTOCOL.to_vec()];

    let quic_crypto = quinn::crypto::rustls::QuicServerConfig::try_from(rustls_config)?;
    let endpoint = quinn::Endpoint::server(
        quinn::ServerConfig::with_crypto(Arc::new(quic_crypto)),
        settings.listen,
    )?;
    tracing::info!(listen = %settings.listen, "QUIC frame server listening");

    let sessions: SessionRegistry = Arc::new(RwLock::new(Vec::new()));
    let mut pump = tokio::spawn(frame_pump(net.frame_rx, sessions.clone()));

    loop {
        tokio::select! {
            _ = &mut pump => {
                break;
            }
            incoming = endpoint.accept() => {
                let Some(incoming) = incoming else {
                    break;
                };
                match incoming.await {
                    Ok(conn) => {
                        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<NetCommand>();
                        let token = Arc::new(());
                        sessions
                            .write()
                            .unwrap()
                            .push(SessionSlot { token: token.clone(), tx });
                        tracing::info!(session = conn.stable_id(), "viewer connected");
                        tokio::spawn(session::handle_connection(
                            conn,
                            rx,
                            net.input_tx.clone(),
                            settings.compression,
                            settings.width,
                            settings.height,
                            token,
                            sessions.clone(),
                        ));
                    }
                    Err(err) => {
                        tracing::debug!(?err, "incoming connection failed");
                    }
                }
            }
        }
    }

    endpoint.close(0u32.into(), b"server shutdown");
    tracing::info!("QUIC frame server stopped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::FrameBuffer;

    /// A 5-frame burst pushed back-to-back must be coalesced to a single
    /// delivery per session (only the newest frame survives).
    #[tokio::test]
    async fn pump_coalesces_frame_bursts() {
        let sessions: SessionRegistry = Arc::new(RwLock::new(Vec::new()));
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<NetCommand>();
        let pump = tokio::spawn(frame_pump(rx, sessions.clone()));

        // One subscribed session.
        let (sess_tx, mut sess_rx) = tokio::sync::mpsc::unbounded_channel();
        sessions.write().unwrap().push(SessionSlot {
            token: Arc::new(()),
            tx: sess_tx,
        });

        // Burst: 5 frames as fast as possible, then shutdown.
        for id in 0..5u64 {
            tx.send(NetCommand::Frame {
                frame: FrameBuffer {
                    data: vec![id as u8; 4],
                    width: 1,
                    height: 1,
                    stride: 4,
                    window_id: 0,
                },
                frame_id: id + 1,
            })
            .unwrap();
        }
        tx.send(NetCommand::Shutdown).unwrap();
        drop(tx);

        pump.await.expect("pump should not panic");

        let mut received = Vec::new();
        while let Ok(cmd) = sess_rx.try_recv() {
            if let NetCommand::Frame { frame, .. } = cmd {
                received.push(frame.data[0] as u64);
            }
        }
        assert_eq!(
            received,
            vec![4],
            "only the newest frame of the burst survives coalescing"
        );
    }
}

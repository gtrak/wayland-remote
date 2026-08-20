//! Headless mode for Linux CI testing — connects, collects frames, prints stats.

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use wayland_remote_protocol::Message;

use crate::session::ViewerSession;
use crate::window_manager::ViewerWindowManager;

pub fn run_headless(
    addr: SocketAddr,
    fingerprint: Option<[u8; 32]>,
    insecure: bool,
    duration_secs: u64,
) -> anyhow::Result<()> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;

    rt.block_on(async {
        let mut session = ViewerSession::connect(addr, fingerprint, insecure).await?;
        eprintln!("headless: connected, {}x{}", session.width, session.height);

        let mut windows = ViewerWindowManager::new();
        let deadline = Instant::now() + Duration::from_secs(duration_secs);
        let mut count = 0u64;
        let mut total = 0u64;

        while Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_secs(2), session.next_frame()).await {
                Ok(Ok(frame)) => {
                    count += 1;
                    total += frame.data.len() as u64;
                    eprintln!(
                        "headless: frame {} (id {}, {}x{}, {} bytes)",
                        count,
                        frame.frame_id,
                        frame.width,
                        frame.height,
                        frame.data.len()
                    );
                }
                Ok(Err(e)) => {
                    eprintln!("headless: frame error: {e}");
                    break;
                }
                Err(_) => {
                    eprintln!("headless: no frame for 2s, retrying");
                }
            }
            // Drain control messages (window lifecycle events, keepalive
            // pings) that accumulated while waiting for the frame.
            while let Some(msg) = session.try_read_control().await {
                match msg {
                    Message::WindowEvent { window_id, event } => {
                        windows.handle_event(window_id, &event);
                        eprintln!(
                            "headless: window {window_id}: {event:?} ({} tracked)",
                            windows.window_count()
                        );
                    }
                    other => {
                        eprintln!("headless: control message: {other:?}");
                    }
                }
            }
        }

        eprintln!("headless: done — {count} frames, {total} bytes in {duration_secs}s");
        session.close();
        Ok(())
    })
}

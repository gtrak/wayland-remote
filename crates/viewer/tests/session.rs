//! Loopback session test: spawn a streaming server and connect a ViewerSession.

use std::net::UdpSocket;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

// @lat: [[tests#Viewer#Session handshake]]
#[test]
fn session_handshake() {
    // Find a free loopback port, releasing the probe socket immediately.
    let probe = UdpSocket::bind("127.0.0.1:0").expect("probe UDP socket should bind");
    let port = probe
        .local_addr()
        .expect("probe socket has a local address")
        .port();
    drop(probe);
    let addr: std::net::SocketAddr = format!("127.0.0.1:{port}").parse().unwrap();

    // Spawn the server in a thread; the QUIC endpoint binds asynchronously
    // inside it, so connects below retry until it is up.
    let shutdown = Arc::new(AtomicBool::new(false));
    let shutdown_flag = shutdown.clone();
    let handle = std::thread::spawn(move || {
        let config = wayland_remote_server::state::Config {
            width: 128,
            height: 128,
            socket_name: Some(format!("wr-viewer-test-{port}")),
            listen: Some(addr),
            compression: wayland_remote_protocol::Compression::Lz4,
            snapshot: None,
        };
        wayland_remote_server::run(config, shutdown_flag, None, None)
    });

    // Connect with retry: the server's QUIC endpoint is not listening yet
    // when this loop starts, and a handshake attempt against a silent port
    // hangs until QUIC's own timeout, so each attempt is bounded by a 2s
    // tokio timeout (same pattern as the server's streaming tests).
    let rt = tokio::runtime::Runtime::new().unwrap();
    let result: anyhow::Result<()> = rt.block_on(async {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            match tokio::time::timeout(
                Duration::from_secs(2),
                wayland_remote_viewer::session::ViewerSession::connect(addr, None, true),
            )
            .await
            {
                Ok(Ok(session)) => {
                    assert_eq!(session.width, 128, "Welcome must report the config width");
                    assert_eq!(session.height, 128, "Welcome must report the config height");
                    session.close();
                    return Ok(());
                }
                Ok(Err(e)) => {
                    assert!(
                        Instant::now() < deadline,
                        "connect failed before the retry deadline: {e:?}"
                    );
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
                Err(_) => {
                    assert!(
                        Instant::now() < deadline,
                        "connect attempts timed out before the retry deadline"
                    );
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    });

    // Shut the server down and assert a clean exit.
    shutdown.store(true, Ordering::Relaxed);
    let server_result = handle.join().expect("server thread should not panic");
    server_result.expect("server should exit cleanly");
    result.expect("session handshake should succeed");
}

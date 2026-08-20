//! xdg-shell window-mapping integration tests (M3).
//!
//! Each test spawns a streaming server, connects a Wayland test client that
//! creates an xdg toplevel, connects a QUIC viewer session, and verifies the
//! `WindowEvent` lifecycle on the viewer's control stream: the
//! initial-configure trap (no `Created` before the client acks and commits)
//! and the full Created/Destroyed round trip.
//!
//! Tests drive their async code with an explicit multi-threaded tokio runtime
//! (same pattern as `streaming.rs`): the compositor thread uses blocking I/O.

mod common;

use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use common::{PATTERN, XdgClient};
use wayland_remote_protocol::{Message, WindowEventKind};
use wayland_remote_server::net::cert::ServerCert;
use wayland_remote_server::run;
use wayland_remote_server::state::Config;
use wayland_remote_viewer::session::ViewerSession;

/// Unique socket name per server instance within this process.
static SOCKET_COUNTER: AtomicUsize = AtomicUsize::new(0);

/// The compositor binds its socket under `$XDG_RUNTIME_DIR`; the test process
/// cannot safely modify the environment (set_var is unsafe in edition 2024),
/// so the variable must be present in the test environment.
fn runtime_dir() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            panic!("XDG_RUNTIME_DIR must be set: compositor sockets are bound there")
        })
}

/// Load or generate the self-signed certificate once per test process.
fn ensure_cert() {
    static INIT: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    INIT.get_or_init(|| {
        ServerCert::load_or_generate().expect("certificate should load or generate");
    });
}

/// Find a free UDP port on loopback, releasing the probe socket immediately.
fn free_port() -> u16 {
    let probe = UdpSocket::bind("127.0.0.1:0").expect("probe UDP socket should bind");
    let port = probe
        .local_addr()
        .expect("probe socket has a local address")
        .port();
    drop(probe);
    port
}

/// Spawn a server thread with QUIC streaming on a free loopback port.
/// Returns the QUIC listen address, the Wayland socket path, the status
/// receiver, the shutdown flag, and the thread handle.
fn spawn_streaming_server() -> (
    SocketAddr,
    PathBuf,
    mpsc::Receiver<usize>,
    Arc<AtomicBool>,
    thread::JoinHandle<anyhow::Result<()>>,
) {
    ensure_cert();
    let ip: std::net::IpAddr = "127.0.0.1"
        .parse()
        .expect("static loopback literal is a valid IP address");
    let listen = SocketAddr::new(ip, free_port());
    let socket_name = format!(
        "wayland-remote-xdg-test-{}",
        SOCKET_COUNTER.fetch_add(1, Ordering::SeqCst)
    );
    let (status_tx, status_rx) = mpsc::channel();
    let shutdown = Arc::new(AtomicBool::new(false));
    let config = Config {
        socket_name: Some(socket_name.clone()),
        listen: Some(listen),
        compression: wayland_remote_protocol::Compression::Lz4,
        ..Config::default()
    };
    let shutdown_flag = shutdown.clone();
    let handle = thread::spawn(move || run(config, shutdown_flag, Some(status_tx), None));
    let socket_path = runtime_dir().join(&socket_name);

    // The socket file is created synchronously at startup; wait for it.
    let deadline = Instant::now() + Duration::from_secs(5);
    while !socket_path.exists() {
        assert!(
            Instant::now() < deadline,
            "server socket did not appear at {socket_path:?}"
        );
        thread::sleep(Duration::from_millis(10));
    }

    (listen, socket_path, status_rx, shutdown, handle)
}

/// Poll the status channel until the reported surface count equals
/// `expected`, failing after a 5 s deadline.
fn wait_for_count(rx: &mpsc::Receiver<usize>, expected: usize) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while let Ok(count) = rx.recv_timeout(Duration::from_millis(500)) {
        if count == expected {
            return;
        }
    }
    panic!("surface count did not reach {expected} before {deadline:?}");
}

/// Signal shutdown and join the server thread, asserting a clean exit.
fn stop_server(shutdown: &Arc<AtomicBool>, handle: thread::JoinHandle<anyhow::Result<()>>) {
    shutdown.store(true, Ordering::SeqCst);
    let result = handle.join().expect("server thread should not panic");
    result.expect("server should exit cleanly");
}

/// Connect a QUIC viewer, retrying while the server's QUIC endpoint is still
/// binding (it starts asynchronously inside the server thread).
fn connect_viewer(runtime: &tokio::runtime::Runtime, addr: SocketAddr) -> ViewerSession {
    let deadline = Instant::now() + Duration::from_secs(10);
    let mut last_err: String;
    loop {
        let attempt = runtime.block_on(async {
            tokio::time::timeout(
                Duration::from_secs(2),
                ViewerSession::connect(addr, None, true),
            )
            .await
        });
        match attempt {
            Ok(Ok(session)) => return session,
            Ok(Err(err)) => last_err = format!("{err}"),
            Err(_) => last_err = "connect attempt timed out".to_owned(),
        }
        if Instant::now() >= deadline {
            panic!("QUIC connect to {addr} failed after retries: {last_err}");
        }
        thread::sleep(Duration::from_millis(50));
    }
}

/// Wait for the next `WindowEvent` on the viewer's control stream, skipping
/// the server's periodic keepalive Pings. Panics after a 10 s deadline.
fn wait_for_window_event(
    runtime: &tokio::runtime::Runtime,
    session: &mut ViewerSession,
    what: &str,
) -> (u64, WindowEventKind) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(
            Instant::now() < deadline,
            "{what} never arrived on the control stream"
        );
        match runtime.block_on(session.try_read_control()) {
            Some(Message::WindowEvent { window_id, event }) => return (window_id, event),
            Some(Message::Ping { .. }) => {}
            Some(other) => panic!("unexpected control message: {other:?}"),
            None => {
                // 10 ms poll timeout; keep waiting until the deadline.
            }
        }
    }
}

#[test]
fn toplevel_lifecycle() {
    // @lat: [[tests#Window Mapping#Toplevel lifecycle]]
    let (listen, socket_path, status_rx, shutdown, handle) = spawn_streaming_server();
    let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
    let mut viewer = connect_viewer(&runtime, listen);

    let mut client = XdgClient::connect_with_toplevel(&socket_path)
        .expect("xdg test client should connect and create a toplevel");
    client
        .ack_and_commit(64, 64, PATTERN)
        .expect("ack + commit should succeed");
    wait_for_count(&status_rx, 1);

    let (window_id, event) = wait_for_window_event(&runtime, &mut viewer, "WindowEvent::Created");
    match event {
        WindowEventKind::Created { width, height, .. } => {
            assert_eq!(width, 64, "Created must report the committed buffer width");
            assert_eq!(
                height, 64,
                "Created must report the committed buffer height"
            );
        }
        other => panic!("expected a Created window event, got {other:?}"),
    }

    client
        .destroy_toplevel()
        .expect("destroy request should flush");
    let (destroyed_id, event) =
        wait_for_window_event(&runtime, &mut viewer, "WindowEvent::Destroyed");
    assert_eq!(
        destroyed_id, window_id,
        "Destroyed must target the same window"
    );
    assert!(
        matches!(event, WindowEventKind::Destroyed),
        "expected a Destroyed window event, got {event:?}"
    );

    drop(client);
    drop(viewer);
    stop_server(&shutdown, handle);
}

#[test]
fn initial_configure_before_created() {
    // @lat: [[tests#Window Mapping#Initial configure before created]]
    let (listen, socket_path, status_rx, shutdown, handle) = spawn_streaming_server();
    let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
    let mut viewer = connect_viewer(&runtime, listen);

    let mut client = XdgClient::connect_with_toplevel(&socket_path)
        .expect("xdg test client should connect and create a toplevel");

    // The toplevel's initial configure is deliberately left unacked and no
    // buffer is committed: for a full second only keepalive Pings may arrive
    // on the control stream — no window event for an unmapped toplevel.
    let start = Instant::now();
    while start.elapsed() < Duration::from_millis(1000) {
        match runtime.block_on(viewer.try_read_control()) {
            Some(Message::Ping { .. }) => {}
            Some(other) => panic!("unexpected control message before mapping: {other:?}"),
            None => {}
        }
    }

    client
        .ack_and_commit(64, 64, PATTERN)
        .expect("ack + commit should succeed");
    wait_for_count(&status_rx, 1);

    let (_window_id, event) = wait_for_window_event(&runtime, &mut viewer, "WindowEvent::Created");
    assert!(
        matches!(event, WindowEventKind::Created { .. }),
        "expected a Created window event, got {event:?}"
    );

    drop(client);
    drop(viewer);
    stop_server(&shutdown, handle);
}

#[test]
fn resized_on_recommit() {
    let (listen, socket_path, status_rx, shutdown, handle) = spawn_streaming_server();
    let runtime = tokio::runtime::Runtime::new().expect("runtime should build");
    let mut viewer = connect_viewer(&runtime, listen);

    let mut client = XdgClient::connect_with_toplevel(&socket_path)
        .expect("xdg test client should connect and create a toplevel");
    client
        .ack_and_commit(64, 64, PATTERN)
        .expect("ack + commit should succeed");
    wait_for_count(&status_rx, 1);

    let (window_id, event) = wait_for_window_event(&runtime, &mut viewer, "WindowEvent::Created");
    assert!(
        matches!(event, WindowEventKind::Created { .. }),
        "expected a Created window event, got {event:?}"
    );

    // Re-commit the mapped toplevel at a larger size: the server must emit a
    // Resized window event carrying the new dimensions.
    client
        .commit_buffer(128, 128, PATTERN)
        .expect("resize commit should succeed");

    let (resized_id, event) = wait_for_window_event(&runtime, &mut viewer, "WindowEvent::Resized");
    assert_eq!(resized_id, window_id, "Resized must target the same window");
    match event {
        WindowEventKind::Resized { width, height } => {
            assert_eq!(width, 128, "Resized must report the new width");
            assert_eq!(height, 128, "Resized must report the new height");
        }
        other => panic!("expected a Resized window event, got {other:?}"),
    }

    drop(client);
    drop(viewer);
    stop_server(&shutdown, handle);
}

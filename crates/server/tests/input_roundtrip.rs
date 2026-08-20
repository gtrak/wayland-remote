//! Input round-trip integration test (plan 006 issue 03).
//!
//! Spawns a streaming server, connects a reactive Wayland test client that
//! binds `wl_pointer` and commits a new buffer on button press, injects a
//! pointer click over QUIC via a viewer session, and asserts the client
//! committed (pixels changed). RED until the input focus fix (issue 05) lands:
//! the client never receives a button event, so no dot commit is made and the
//! frame never changes.
//!
//! Tests drive their async code with an explicit multi-threaded tokio runtime
//! (same pattern as `xdg.rs`): the compositor thread uses blocking I/O.

mod common;

use std::net::{SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use common::{PATTERN, XdgClient};
use wayland_remote_protocol::{ButtonState, InputEvent, Message, WindowEventKind};
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
        "wayland-remote-input-test-{}",
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

/// A second fill color (opaque red) the client commits on button press.
/// `0xFFFF0000` = A=FF, R=FF, G=00, B=00; unambiguously distinct from
/// [`PATTERN`] (opaque blue) in any channel layout.
const DOT_COLOR: u32 = 0xFFFF0000;

#[test]
fn pointer_click_round_trip() {
    // @lat: [[tests#Input round-trip#Pointer click round-trip]]
    let (listen, socket_path, status_rx, shutdown, handle) = spawn_streaming_server();
    let runtime = tokio::runtime::Runtime::new().expect("runtime should build");

    // Connect the viewer before the client maps the window: window events are
    // broadcast only to sessions already connected, so a late viewer would miss
    // the one-shot `Created` event.
    let mut viewer = connect_viewer(&runtime, listen);

    // Create the test client, ack configure, and commit a base-color buffer.
    let mut client =
        XdgClient::connect_with_toplevel(&socket_path).expect("xdg test client should connect");
    client
        .ack_and_commit(64, 64, PATTERN)
        .expect("ack + commit should succeed");
    wait_for_count(&status_rx, 1);

    // Bind wl_pointer so the client can receive pointer events.
    let pointer_data = client.bind_pointer();

    // Wait for the window to be created.
    let (window_id, event) = wait_for_window_event(&runtime, &mut viewer, "WindowEvent::Created");
    assert!(
        matches!(event, WindowEventKind::Created { .. }),
        "expected Created, got {event:?}"
    );

    // Capture a baseline frame (the client's committed PATTERN color).
    let baseline = runtime.block_on(async {
        tokio::time::timeout(Duration::from_secs(5), viewer.next_frame())
            .await
            .expect("baseline frame should arrive")
            .expect("frame should decode")
    });

    // Inject a pointer click: move to center, press, release.
    runtime.block_on(async {
        viewer
            .send_input(window_id, InputEvent::PointerMove { x: 32.0, y: 32.0 })
            .await
            .expect("pointer move should send");
        viewer
            .send_input(
                window_id,
                InputEvent::PointerButton {
                    button: 272, // BTN_LEFT
                    state: ButtonState::Pressed,
                },
            )
            .await
            .expect("button press should send");
        viewer
            .send_input(
                window_id,
                InputEvent::PointerButton {
                    button: 272,
                    state: ButtonState::Released,
                },
            )
            .await
            .expect("button release should send");
    });

    // Give the server time to inject the input, then poll the client's event
    // queue (non-blocking) to detect a button press on the bound pointer.
    thread::sleep(Duration::from_millis(200));
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut button_received = false;
    while Instant::now() < deadline {
        client
            .dispatch_pending()
            .expect("dispatching pending events should not fail");
        if let Ok(mut click) = pointer_data.click.lock() {
            if click.is_some() {
                *click = None;
                button_received = true;
                break;
            }
        }
        thread::sleep(Duration::from_millis(50));
    }

    // If the client received a button press, commit a new buffer with a
    // different color (the "dot"). This is what the pixel-change assertion
    // checks for.
    if button_received {
        client
            .commit_buffer(64, 64, DOT_COLOR)
            .expect("dot commit should succeed");
    }

    // Read frames and check if pixels changed from the baseline.
    let mut pixels_changed = false;
    let frame_deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < frame_deadline {
        match runtime.block_on(async {
            tokio::time::timeout(Duration::from_secs(2), viewer.next_frame()).await
        }) {
            Ok(Ok(frame)) => {
                if frame.data != baseline.data {
                    pixels_changed = true;
                    break;
                }
            }
            _ => break,
        }
    }

    // Drain control messages (keepalive pings).
    while runtime.block_on(viewer.try_read_control()).is_some() {}

    drop(client);
    drop(viewer);
    stop_server(&shutdown, handle);

    assert!(
        button_received,
        "client never received a pointer button event — input is not reaching the client"
    );
    assert!(
        pixels_changed,
        "pixels never changed after the click — the client's dot commit was not rendered"
    );
}

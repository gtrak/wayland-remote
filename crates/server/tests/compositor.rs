//! Integration tests for the headless Smithay compositor (plan 001 issue 03).

mod common;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use common::TestClient;
use wayland_remote_server::run;
use wayland_remote_server::state::Config;

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

/// Spawn a server thread listening on a unique socket. Returns the socket
/// path, the status channel receiver, the shutdown flag, and the thread
/// handle.
fn spawn_server() -> (
    PathBuf,
    mpsc::Receiver<usize>,
    Arc<AtomicBool>,
    thread::JoinHandle<anyhow::Result<()>>,
) {
    let socket_name = format!(
        "wayland-remote-test-{}",
        SOCKET_COUNTER.fetch_add(1, Ordering::SeqCst)
    );
    let (status_tx, status_rx) = mpsc::channel();
    let shutdown = Arc::new(AtomicBool::new(false));
    let config = Config {
        socket_name: Some(socket_name.clone()),
        ..Config::default()
    };
    let shutdown_flag = shutdown.clone();
    let handle = thread::spawn(move || run(config, shutdown_flag, Some(status_tx)));
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

    (socket_path, status_rx, shutdown, handle)
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
    panic!("surface count did not reach {expected} by {deadline:?}");
}

/// Signal shutdown and join the server thread, asserting a clean exit.
fn stop_server(shutdown: &Arc<AtomicBool>, handle: thread::JoinHandle<anyhow::Result<()>>) {
    shutdown.store(true, Ordering::SeqCst);
    let result = handle.join().expect("server thread should not panic");
    result.expect("server should exit cleanly");
}

#[test]
fn client_connects_and_creates_surface() {
    // @lat: [[tests#Compositor#Client connects and creates surface]]
    let (socket_path, rx, shutdown, handle) = spawn_server();

    let client = TestClient::connect_and_create_surface(&socket_path)
        .expect("test client should connect and commit a surface");
    wait_for_count(&rx, 1);
    drop(client);

    stop_server(&shutdown, handle);
}

#[test]
fn multiple_clients_supported() {
    // @lat: [[tests#Compositor#Multiple clients supported]]
    let (socket_path, rx, shutdown, handle) = spawn_server();

    let first =
        TestClient::connect_and_create_surface(&socket_path).expect("first client should connect");
    wait_for_count(&rx, 1);

    let second =
        TestClient::connect_and_create_surface(&socket_path).expect("second client should connect");
    wait_for_count(&rx, 2);

    drop(first);
    drop(second);
    wait_for_count(&rx, 0);

    stop_server(&shutdown, handle);
}

#[test]
fn client_disconnect_cleans_up() {
    // @lat: [[tests#Compositor#Client disconnect cleans up]]
    let (socket_path, rx, shutdown, handle) = spawn_server();

    let client = TestClient::connect_and_create_surface(&socket_path)
        .expect("test client should connect and commit a surface");
    wait_for_count(&rx, 1);

    drop(client);
    wait_for_count(&rx, 0);

    stop_server(&shutdown, handle);
}

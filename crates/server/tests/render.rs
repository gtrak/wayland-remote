//! Integration tests for offscreen pixman rendering (plan 001 issue 04).
//!
//! Each test spawns the server in-process on a unique socket, drives it with a
//! real Wayland test client, requests a render over a channel, and inspects the
//! read-back BGRA [`FrameBuffer`].

mod common;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::{Duration, Instant};

use common::{TestClient, argb_to_bgra};
use wayland_remote_server::rendering::{FrameBuffer, RenderRequest};
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

/// Spawn a server thread listening on a unique socket. Returns the socket path,
/// the status receiver, the render-request sender, the shutdown flag, and the
/// thread handle.
#[allow(clippy::type_complexity)]
fn spawn_server() -> (
    PathBuf,
    Receiver<usize>,
    Sender<RenderRequest>,
    Arc<AtomicBool>,
    thread::JoinHandle<anyhow::Result<()>>,
) {
    let socket_name = format!(
        "wayland-remote-render-{}",
        SOCKET_COUNTER.fetch_add(1, Ordering::SeqCst)
    );
    let (status_tx, status_rx) = mpsc::channel();
    let (render_tx, render_rx) = mpsc::channel();
    let shutdown = Arc::new(AtomicBool::new(false));
    let config = Config {
        socket_name: Some(socket_name.clone()),
        ..Config::default()
    };
    let shutdown_flag = shutdown.clone();
    let handle =
        thread::spawn(move || run(config, shutdown_flag, Some(status_tx), Some(render_rx)));
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

    (socket_path, status_rx, render_tx, shutdown, handle)
}

/// Poll the status channel until the reported surface count equals
/// `expected`, failing after a 5 s deadline.
fn wait_for_count(rx: &Receiver<usize>, expected: usize) {
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

/// Send a single render request and return the rendered [`FrameBuffer`].
fn request_render(tx: &Sender<RenderRequest>) -> anyhow::Result<FrameBuffer> {
    let (reply_tx, reply_rx) = mpsc::channel();
    tx.send(RenderRequest::Render { reply: reply_tx })?;
    reply_rx
        .recv_timeout(Duration::from_secs(5))
        .map_err(|e| anyhow::anyhow!("render reply timeout: {e}"))
}

/// Poll render requests until `pred(frame)` holds, failing after a 5 s
/// deadline. Re-requesting tolerates a render that captured a stale frame
/// before a commit was dispatched.
fn wait_for_render(
    tx: &Sender<RenderRequest>,
    pred: impl Fn(&FrameBuffer) -> bool,
) -> anyhow::Result<FrameBuffer> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let frame = request_render(tx)?;
        if pred(&frame) {
            return Ok(frame);
        }
        if Instant::now() >= deadline {
            anyhow::bail!("render never satisfied the predicate within 5s");
        }
        thread::sleep(Duration::from_millis(25));
    }
}

/// The BGRA byte at `(x, y)` of a frame.
fn pixel(frame: &FrameBuffer, x: u32, y: u32) -> [u8; 4] {
    let stride = frame.stride as usize;
    let off = (y as usize) * stride + (x as usize) * 4;
    [
        frame.data[off],
        frame.data[off + 1],
        frame.data[off + 2],
        frame.data[off + 3],
    ]
}

/// The expected BGRA bytes of the client's blue fill pattern.
const BLUE: [u8; 4] = [0xFF, 0x00, 0x00, 0xFF];
/// Opaque background black.
const BLACK: [u8; 4] = [0x00, 0x00, 0x00, 0xFF];

#[test]
fn renders_client_pattern() {
    // @lat: [[tests#Rendering#Renders client pattern]]
    let (socket_path, status_rx, render_tx, shutdown, handle) = spawn_server();

    let client = TestClient::connect_and_create_surface(&socket_path)
        .expect("test client should connect and commit a surface");
    wait_for_count(&status_rx, 1);

    let frame =
        wait_for_render(&render_tx, |f| pixel(f, 0, 0) == BLUE).expect("render should complete");

    // The client committed a 64x64 blue surface at the (0,0) layout slot; the
    // top-left pixel must match the pattern exactly in BGRA byte order.
    assert_eq!(
        pixel(&frame, 0, 0),
        BLUE,
        "pixel (0,0) should be the client pattern"
    );
    assert_eq!(
        pixel(&frame, 63, 63),
        BLUE,
        "pixel (63,63) is inside the 64x64 surface"
    );
    assert_eq!(
        pixel(&frame, 64, 64),
        BLACK,
        "pixel (64,64) is just outside the 64x64 surface"
    );

    drop(client);
    stop_server(&shutdown, handle);
}

#[test]
fn bgra_byte_order() {
    // @lat: [[tests#Rendering#BGRA byte order]]
    let (socket_path, status_rx, render_tx, shutdown, handle) = spawn_server();

    let client = TestClient::connect_and_create_surface(&socket_path)
        .expect("test client should connect and commit a surface");
    wait_for_count(&status_rx, 1);

    let frame =
        wait_for_render(&render_tx, |f| pixel(f, 0, 0) == BLUE).expect("render should complete");

    // The client filled with 0xFF0000FF (A=FF, R=00, G=00, B=FF). In an
    // Argb8888 buffer on a little-endian host that is stored as BGRA
    // [B, G, R, A] = [FF, 00, 00, FF]. Assert the exact byte order and rule out
    // the RGBA layout ([R, G, B, A] would be [00, 00, FF, FF]).
    let expected = argb_to_bgra(0xFF0000FF);
    assert_eq!(
        frame.stride as u32,
        frame.width * 4,
        "contiguous readback stride"
    );
    assert_eq!(
        pixel(&frame, 0, 0),
        expected,
        "byte order must be [B, G, R, A]"
    );
    assert_ne!(
        pixel(&frame, 0, 0),
        [0x00, 0x00, 0xFF, 0xFF],
        "byte order must not be [R, G, B, A]"
    );

    drop(client);
    stop_server(&shutdown, handle);
}

#[test]
fn handles_surface_resize() {
    // @lat: [[tests#Rendering#Handles surface resize]]
    let (socket_path, status_rx, render_tx, shutdown, handle) = spawn_server();

    let mut client = TestClient::connect_and_create_surface(&socket_path)
        .expect("test client should connect and commit a 64x64 surface");
    wait_for_count(&status_rx, 1);

    // While the surface is 64x64, a pixel beyond that region is background
    // black (the surface sits at the (0,0) layout slot).
    let small = wait_for_render(&render_tx, |f| pixel(f, 100, 100) == BLACK)
        .expect("render should complete");
    assert_eq!(
        pixel(&small, 100, 100),
        BLACK,
        "outside the 64x64 surface is black"
    );

    // Re-commit the same surface as 128x128.
    client
        .commit_buffer(128, 128, common::PATTERN)
        .expect("resize commit should succeed");

    // Poll until the read-back reflects the new size: pixel (100,100) is inside
    // the 128x128 surface and should now be the blue pattern.
    let resized =
        wait_for_render(&render_tx, |f| pixel(f, 100, 100) == BLUE).expect("resize render");
    assert_eq!(pixel(&resized, 0, 0), BLUE, "top-left stays the pattern");
    assert_eq!(
        pixel(&resized, 100, 100),
        BLUE,
        "(100,100) is inside the resized 128x128 surface"
    );
    assert_eq!(
        pixel(&resized, 128, 128),
        BLACK,
        "(128,128) is just outside the resized surface"
    );

    drop(client);
    stop_server(&shutdown, handle);
}

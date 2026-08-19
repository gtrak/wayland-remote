//! Test client harness for the headless compositor integration tests.
//!
//! Shared by `compositor.rs` and `render.rs`; not every helper is used by
//! every test binary, so dead-code is allowed here.
#![allow(dead_code)]

use std::io::{Seek, SeekFrom, Write};
use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;
use std::path::Path;

use wayland_client::globals::{GlobalListContents, registry_queue_init};
use wayland_client::protocol::wl_buffer::WlBuffer;
use wayland_client::protocol::wl_compositor::WlCompositor;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};

/// The known fill color committed by the test client: opaque blue
/// (`A=0xFF, R=0x00, G=0x00, B=0xFF`). Picked so its BGRA byte order
/// (`[FF, 00, 00, FF]`) is unambiguously distinguishable from an RGBA layout.
pub const PATTERN: u32 = 0xFF0000FF;

/// Convert an `0xAARRGGBB` color to its little-endian in-memory BGRA bytes.
#[must_use]
pub fn argb_to_bgra(color: u32) -> [u8; 4] {
    [
        (color & 0xFF) as u8,
        ((color >> 8) & 0xFF) as u8,
        ((color >> 16) & 0xFF) as u8,
        ((color >> 24) & 0xFF) as u8,
    ]
}

/// Event-queue state for the test client. The client never processes
/// protocol events, so every dispatch is a no-op.
#[derive(Debug, Default)]
struct TestClientState;

/// `registry_queue_init` requires a registry dispatch for the queue's state
/// type. The initial global list is collected during init's roundtrip and no
/// dynamic globals appear in these tests, so this is a no-op.
impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for TestClientState {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

wayland_client::delegate_noop!(TestClientState: WlCompositor);
wayland_client::delegate_noop!(TestClientState: WlShm);
wayland_client::delegate_noop!(TestClientState: WlShmPool);
wayland_client::delegate_noop!(TestClientState: WlBuffer);
wayland_client::delegate_noop!(TestClientState: WlSeat);
wayland_client::delegate_noop!(TestClientState: WlSurface);

/// A minimal Wayland client: connects, binds the core globals (compositor,
/// shm, seat), and commits an shm surface. Pools/buffers are kept alive so the
/// shared memory backing them stays mapped; dropping the client disconnects.
pub struct TestClient {
    conn: Connection,
    _queue: EventQueue<TestClientState>,
    _compositor: WlCompositor,
    _shm: WlShm,
    _seat: WlSeat,
    _pools: Vec<WlShmPool>,
    _buffers: Vec<WlBuffer>,
    surface: WlSurface,
}

impl TestClient {
    /// Connect to the compositor socket at `socket_path`, bind the core
    /// globals, and commit a 64x64 ARGB8888 surface filled with [`PATTERN`].
    pub fn connect_and_create_surface(socket_path: &Path) -> anyhow::Result<Self> {
        let stream = UnixStream::connect(socket_path)?;
        let conn = Connection::from_socket(stream)?;
        let (globals, queue) = registry_queue_init::<TestClientState>(&conn)?;
        let qh = queue.handle();

        let compositor: WlCompositor = globals.bind(&qh, 1..=5, ())?;
        let shm: WlShm = globals.bind(&qh, 1..=1, ())?;
        let seat: WlSeat = globals.bind(&qh, 1..=7, ())?;
        let surface = compositor.create_surface(&qh, ());

        let mut client = Self {
            conn,
            _queue: queue,
            _compositor: compositor,
            _shm: shm,
            _seat: seat,
            _pools: Vec::new(),
            _buffers: Vec::new(),
            surface,
        };
        // Commit an initial 64x64 surface filled with the known pattern.
        client.commit_buffer(64, 64, PATTERN)?;
        Ok(client)
    }

    /// Create an ARGB8888 shm pool + `width`x`height` buffer filled with
    /// `color`, attach it to the surface, and commit. The pool/buffer are kept
    /// alive so their shared memory remains mapped.
    pub fn commit_buffer(&mut self, width: u32, height: u32, color: u32) -> anyhow::Result<()> {
        let qh = self._queue.handle();

        let mut file = tempfile::tempfile()?;
        let mut pixels = Vec::with_capacity((width * height * 4) as usize);
        for _ in 0..(width * height) {
            pixels.extend_from_slice(&color.to_ne_bytes());
        }
        file.write_all(&pixels)?;
        file.seek(SeekFrom::Start(0))?;

        let pool = self
            ._shm
            .create_pool(file.as_fd(), (width * height * 4) as i32, &qh, ());
        let buffer = pool.create_buffer(
            0,
            width as i32,
            height as i32,
            (width * 4) as i32,
            wl_shm::Format::Argb8888,
            &qh,
            (),
        );

        self.surface.attach(Some(&buffer), 0, 0);
        self.surface.commit();
        self.conn.flush()?;

        self._pools.push(pool);
        self._buffers.push(buffer);
        Ok(())
    }
}

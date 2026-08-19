//! Test client harness for the headless compositor integration tests.

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
/// shm, seat), and commits a 64x64 shm surface. Dropping it disconnects the
/// client, which should clean up the server's surface tracking.
pub struct TestClient {
    _conn: Connection,
    _queue: EventQueue<TestClientState>,
    _compositor: WlCompositor,
    _shm: WlShm,
    _seat: WlSeat,
    _pool: WlShmPool,
    _buffer: WlBuffer,
    _surface: WlSurface,
}

impl TestClient {
    /// Connect to the compositor socket at `socket_path` and commit a
    /// 64x64 ARGB8888 surface filled with a recognizable pattern.
    pub fn connect_and_create_surface(socket_path: &Path) -> anyhow::Result<Self> {
        let stream = UnixStream::connect(socket_path)?;
        let conn = Connection::from_socket(stream)?;
        let (globals, queue) = registry_queue_init::<TestClientState>(&conn)?;
        let qh = queue.handle();

        let compositor: WlCompositor = globals.bind(&qh, 1..=5, ())?;
        let shm: WlShm = globals.bind(&qh, 1..=1, ())?;
        let seat: WlSeat = globals.bind(&qh, 1..=7, ())?;

        // 64x64 ARGB8888 buffer filled with a recognizable pattern.
        let mut file = tempfile::tempfile()?;
        let mut pixels = Vec::with_capacity(64 * 64 * 4);
        for _ in 0..64 * 64 {
            pixels.extend_from_slice(&0xFFAABBCCu32.to_ne_bytes());
        }
        file.write_all(&pixels)?;
        file.seek(SeekFrom::Start(0))?;

        let pool = shm.create_pool(file.as_fd(), 64 * 64 * 4, &qh, ());
        let buffer = pool.create_buffer(0, 64, 64, 256, wl_shm::Format::Argb8888, &qh, ());

        let surface = compositor.create_surface(&qh, ());
        surface.attach(Some(&buffer), 0, 0);
        surface.commit();
        conn.flush()?;

        Ok(Self {
            _conn: conn,
            _queue: queue,
            _compositor: compositor,
            _shm: shm,
            _seat: seat,
            _pool: pool,
            _buffer: buffer,
            _surface: surface,
        })
    }
}

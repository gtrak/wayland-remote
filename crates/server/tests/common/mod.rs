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
use wayland_client::protocol::wl_output::WlOutput;
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_seat::WlSeat;
use wayland_client::protocol::wl_shm::{self, WlShm};
use wayland_client::protocol::wl_shm_pool::WlShmPool;
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, EventQueue, QueueHandle};
use wayland_protocols::xdg::shell::client::xdg_surface::{self, XdgSurface};
use wayland_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel;
use wayland_protocols::xdg::shell::client::xdg_wm_base::XdgWmBase;

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

/// Event-queue state for the xdg-shell test client. Configure events are
/// acknowledged on dispatch (the xdg-shell initial-configure trap); every
/// other event is a no-op.
#[derive(Debug, Default)]
struct XdgClientState;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for XdgClientState {
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

wayland_client::delegate_noop!(XdgClientState: WlCompositor);
wayland_client::delegate_noop!(XdgClientState: ignore WlShm);
wayland_client::delegate_noop!(XdgClientState: WlShmPool);
wayland_client::delegate_noop!(XdgClientState: WlBuffer);
wayland_client::delegate_noop!(XdgClientState: ignore WlSurface);
wayland_client::delegate_noop!(XdgClientState: ignore WlOutput);
wayland_client::delegate_noop!(XdgClientState: ignore WlSeat);
wayland_client::delegate_noop!(XdgClientState: ignore XdgWmBase);
wayland_client::delegate_noop!(XdgClientState: ignore XdgToplevel);
impl Dispatch<XdgSurface, ()> for XdgClientState {
    fn event(
        _: &mut Self,
        xdg: &XdgSurface,
        event: xdg_surface::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let xdg_surface::Event::Configure { serial } = event {
            xdg.ack_configure(serial);
        }
    }
}

/// A Wayland client with an xdg toplevel. Unlike [`TestClient`], creating the
/// toplevel does not commit anything: the xdg-shell initial-configure trap
/// keeps the window unmapped until the client acks a configure and commits a
/// buffer, which is what [`ack_and_commit`](XdgClient::ack_and_commit) does.
pub struct XdgClient {
    conn: Connection,
    _queue: EventQueue<XdgClientState>,
    _compositor: WlCompositor,
    _shm: WlShm,
    _wm_base: XdgWmBase,
    _surface: WlSurface,
    _xdg_surface: XdgSurface,
    toplevel: XdgToplevel,
    _pools: Vec<WlShmPool>,
    _buffers: Vec<WlBuffer>,
}

impl XdgClient {
    /// Connect to `socket_path`, bind compositor/shm/xdg_wm_base, and create
    /// an xdg toplevel on a fresh surface.
    ///
    /// No events are dispatched and no buffer is committed: the toplevel's
    /// initial configure stays unacked, so the server cannot map the window
    /// yet. Dropping the client destroys the toplevel.
    pub fn connect_with_toplevel(socket_path: &Path) -> anyhow::Result<Self> {
        let stream = UnixStream::connect(socket_path)?;
        let conn = Connection::from_socket(stream)?;
        let (globals, queue) = registry_queue_init::<XdgClientState>(&conn)?;
        let qh = queue.handle();

        let compositor: WlCompositor = globals.bind(&qh, 1..=5, ())?;
        let shm: WlShm = globals.bind(&qh, 1..=1, ())?;
        let wm_base: XdgWmBase = globals.bind(&qh, 1..=6, ())?;
        let surface = compositor.create_surface(&qh, ());
        let xdg_surface = wm_base.get_xdg_surface(&surface, &qh, ());
        let toplevel = xdg_surface.get_toplevel(&qh, ());
        conn.flush()?;

        Ok(Self {
            conn,
            _queue: queue,
            _compositor: compositor,
            _shm: shm,
            _wm_base: wm_base,
            _surface: surface,
            _xdg_surface: xdg_surface,
            toplevel,
            _pools: Vec::new(),
            _buffers: Vec::new(),
        })
    }

    /// Block until the initial configure arrives and is acknowledged, then commit a
    /// `width`x`height` buffer filled with `color` on the toplevel surface.
    ///
    /// The first commit after the ack is what maps the window on the server
    /// side and makes it emit a `Created` window event.
    pub fn ack_and_commit(&mut self, width: u32, height: u32, color: u32) -> anyhow::Result<()> {
        // Block until the server's initial configure arrives and is acked;
        // without it the commit would map nothing. The server always sends
        // the initial configure from its new_toplevel handler, so this
        // cannot hang while the server is alive.
        self._queue.blocking_dispatch(&mut XdgClientState)?;
        self.commit_buffer(width, height, color)
    }

    /// Create an ARGB8888 shm pool + buffer filled with `color`, attach it to
    /// the toplevel surface, and commit. Pools/buffers are kept alive so their
    /// shared memory stays mapped. A re-commit at a new size is how tests
    /// exercise mapped-window resizes.
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

        self._surface.attach(Some(&buffer), 0, 0);
        self._surface.commit();
        self.conn.flush()?;

        self._pools.push(pool);
        self._buffers.push(buffer);
        Ok(())
    }

    /// Send `xdg_toplevel.destroy` and flush; the server emits a
    /// `Destroyed` window event for the window.
    pub fn destroy_toplevel(&mut self) -> anyhow::Result<()> {
        self.toplevel.destroy();
        self.conn.flush()?;
        Ok(())
    }
}

---
name: smithay-07-api
description: >-
  Exact smithay 0.7 / wayland-server 0.31 / calloop 0.14 API contract for the
  wayland-remote headless compositor. Researched against docs.rs and the
  Smallvil/anvil reference compositor. Read this before writing compositor
  code — do NOT re-research the API.
---

# Smithay 0.7 API Contract for wayland-remote

Exact types, signatures, and wiring patterns verified against smithay 0.7.0,
wayland-server 0.31.14, and calloop 0.14.4. Use these directly; do not
re-derive from docs.rs.

## Socket

wayland-server 0.31 **removed** `Display::add_socket_auto` /
`create_event_source`. Use smithay's calloop-integrated socket source:

```rust
use smithay::wayland::socket::ListeningSocketSource;

// Auto-generated name (wayland-0, wayland-1, ...):
let socket_source = ListeningSocketSource::new_auto()?;

// Or named:
let socket_source = ListeningSocketSource::with_name("wayland-remote")?;

// Get the socket name (e.g. "wayland-remote") — print $XDG_RUNTIME_DIR/<name>
let socket_name = socket_source.socket_name().to_owned();
```

On accept, the callback receives a `UnixStream`:

```rust
handle.insert_source(socket_source, |stream, &mut state, _| {
    let client_data = Arc::new(ClientState) as Arc<dyn wayland_server::backend::ClientData>;
    state.display_handle.insert_client(stream, client_data)
        .expect("Failed to insert client");
})?;
```

The socket file + lockfile are auto-removed on Drop.

## Display + Event Loop

```rust
use wayland_server::Display;
use calloop::EventLoop;

let mut display: Display<State> = Display::new()?;
let display_handle = display.handle();

let mut event_loop: EventLoop<State> = EventLoop::try_new()?;
let handle = event_loop.handle();

// ... insert sources into handle ...

// Run with a timeout; the callback is the idle/data callback
event_loop.run(
    Some(std::time::Duration::from_millis(50)),
    &mut state,
    |_state| {
        // idle callback — check shutdown here if needed
    },
)?;
```

### Dispatching client events

Use a **timer source** (avoids `unsafe` — workspace denies `unsafe_code`):

```rust
use calloop::timer::{Timer, Timeout};

let timer = Timer::immediate().unwrap(); // fires immediately, then every interval
handle.insert_source(timer, |_, _, state| {
    state.display.dispatch_clients(state)?;
    state.display.flush_clients()?;
    Ok(calloop::PostAction::Continue)
})?;
```

Actually — `Display` needs to be in State or accessible. Better pattern: store
the `DisplayHandle` in State (which you already do), and store the `Display`
itself alongside the event loop. The dispatch call is
`display.dispatch_clients(&mut state)`.

NOTE: `dispatch_clients` takes `&mut self` on Display and `&mut State`. So
Display must be owned outside State or State must not borrow from Display.
Pattern: own `Display<State>` in a local variable, pass `&display.handle()` to
State, call `display.dispatch_clients(&mut state)` in the timer callback.
Since the timer callback closure captures `&mut display`, this works if
Display lives as long as the event loop run.

Alternative cleaner pattern: don't use a timer. Use the event loop's own run
loop:

```rust
loop {
    event_loop.dispatch(Some(Duration::from_millis(50)), &mut state)?;
    display.dispatch_clients(&mut state)?;
    display.flush_clients()?;
    if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
        break;
    }
}
```

This is simpler and avoids closure capture issues. Use this.

### SIGINT

```rust
use calloop::signals::{Signals, Signal};

// Requires calloop feature "signals" — add to Cargo.toml:
// calloop = { workspace = true, features = ["signals"] }

let signals = Signals::new(&[Signal::SIGINT, Signal::SIGTERM])?;
handle.insert_source(signals, |_, _, state| {
    // set shutdown flag
    Ok(calloop::PostAction::Continue)
})?;
```

## Compositor

```rust
use smithay::wayland::compositor::{CompositorState, CompositorHandler, with_states};
use smithay::wayland::compositor::SurfaceAttributes;

// In State init:
let compositor_state = CompositorState::new::<State>(&display_handle);

// Handler impl:
impl CompositorHandler for State {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(
        &mut self,
        client: &'a wayland_server::Client,
    ) -> &'a smithay::wayland::compositor::CompositorClientState {
        // This returns the client's compositor state from the ClientData
        client.get_data::<ClientState>().unwrap().compositor_state.get()
    }

    fn commit(&mut self, surface: &wayland_server::protocol::wl_surface::WlSurface) {
        // Read committed surface attributes:
        let (width, height) = with_states(surface, |states| {
            let attrs = states.cached_state.get::<SurfaceAttributes>().current();
            if let Some(buffer) = &attrs.buffer {
                let (w, h) = buffer.dimensions();
                (Some(w), Some(h))
            } else {
                (None, None)
            }
        }).unwrap_or((None, None));

        // Record surface in map:
        let id = surface.id();
        self.surfaces.insert(id, ());
        if let Some(tx) = &self.status_tx {
            let _ = tx.send(self.surfaces.len());
        }

        tracing::debug!(?width, ?height, "surface commit");
    }

    fn destroyed(&mut self, surface: &wayland_server::protocol::wl_surface::WlSurface) {
        let id = surface.id();
        if self.surfaces.remove(&id).is_some() {
            if let Some(tx) = &self.status_tx {
                let _ = tx.send(self.surfaces.len());
            }
        }
    }
}

delegate_compositor!(State);
```

### ClientState (for compositor client data)

```rust
use smithay::wayland::compositor::CompositorClientState;
use wayland_server::backend::ClientData;

pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {}

impl std::ops::Deref for ClientState {
    type Target = CompositorClientState;
    fn deref(&self) -> &Self::Target { &self.compositor_state }
}
```

NOTE: `client_compositor_state` lifetime may be tricky. If the compiler
complains, check the exact signature in smithay 0.7 — it may return
`&CompositorClientState` from the client data via `Deref`. Adapt to whatever
the delegate macro's trait bound requires.

## SHM

```rust
use smithay::wayland::shm::{ShmState, ShmHandler};
use smithay::wayland::buffer::BufferHandler;

// Init:
let shm_state = ShmState::new::<State>(&display_handle, vec![]);

// Handlers:
impl ShmHandler for State {
    fn shm_state(&mut self) -> &mut ShmState { &mut self.shm_state }
}

impl BufferHandler for State {
    fn buffer_destroyed(&mut self, _buffer: &wayland_server::protocol::wl_buffer::WlBuffer) {
        // cleanup if needed
    }
}

delegate_shm!(State);
```

## Seat

```rust
use smithay::wayland::seat::{Seat, SeatState, SeatHandler};
use smithay::input::keyboard::XkbConfig;

// Init:
let seat_state = SeatState::<State>::new();
let mut seat = seat_state.new_wl_seat(&display_handle, "wayland-remote");
seat.add_keyboard(XkbConfig::default(), 25, 600)?;
seat.add_pointer();

// Handler:
impl SeatHandler for State {
    type KeyboardFocus = wayland_server::protocol::wl_surface::WlSurface;
    type PointerFocus = wayland_server::protocol::wl_surface::WlSurface;
    type TouchFocus = wayland_server::protocol::wl_surface::WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<State> { &mut self.seat_state }

    fn focus_changed(&mut self, _seat: &Seat<State>, _focused: Option<&WlSurface>) {}
    fn cursor_image(&mut self, _seat: &Seat<State>, _image: smithay::wayland::seat::CursorImageStatus) {}
}

delegate_seat!(State);
```

NOTE: The `SeatHandler` trait may have slightly different associated types or
methods in 0.7. Check compiler errors and adapt. The key things: `seat_state()`
method, and the three focus type aliases.

## Output

```rust
use smithay::output::{Output, PhysicalProperties, Mode, Subpixel};
use smithay::utils::{Transform, Scale, Rectangle};

// Create output:
let output = Output::new(
    "wayland-remote".into(),
    PhysicalProperties {
        size: (1280, 720).into(),  // mm (doesn't matter for headless)
        subpixel: Subpixel::Unknown,
        make: "wayland-remote".into(),
        model: "headless".into(),
    },
);

// Set mode:
let mode = Mode {
    size: (config.width, config.height).into(),
    refresh: 60000,  // 60 Hz in millihertz
};
output.set_preferred(mode);
output.change_current_state(
    Some(mode),
    Some(Transform::Normal),
    Some(Scale::Integer(1)),
    Some((0, 0).into()),
);
output.create_global::<State>(&display_handle);

// Handler (minimal — output_bound is the only method, has a default impl):
impl smithay::wayland::output::OutputHandler for State {
    // OutputHandler has a default impl in 0.7; may not need any methods.
    // If the delegate macro requires it, add:
    // fn output_bound(&mut self, _output: &Output, _global: &...) {}
}

delegate_output!(State);
```

NOTE: `delegate_output!` may require `OutputManagerState` — if the compiler
demands it, add `output_manager_state: OutputManagerState` to State and init
with `OutputManagerState::new()`.

## State struct (complete)

```rust
use std::collections::HashMap;
use std::sync::mpsc::Sender;
use wayland_server::{DisplayHandle, Client, backend::ObjectId};
use wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::compositor::CompositorState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::seat::{Seat, SeatState};
use smithay::output::Output;

pub struct Config {
    pub width: u32,
    pub height: u32,
    pub socket_name: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self { width: 1280, height: 720, socket_name: None }
    }
}

pub struct State {
    pub display_handle: DisplayHandle,
    pub compositor_state: CompositorState,
    pub shm_state: ShmState,
    pub seat_state: SeatState<State>,
    pub seat: Seat<State>,
    pub output: Output,
    pub surfaces: HashMap<ObjectId, ()>,
    pub config: Config,
    pub status_tx: Option<Sender<usize>>,
}

impl State {
    pub fn surface_count(&self) -> usize { self.surfaces.len() }
}
```

## Imports (the full use block)

```rust
use smithay::delegate_compositor;
use smithay::delegate_shm;
use smithay::delegate_seat;
use smithay::delegate_output;
use smithay::wayland::compositor::{CompositorState, CompositorHandler, with_states, SurfaceAttributes, CompositorClientState};
use smithay::wayland::shm::{ShmState, ShmHandler};
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::seat::{Seat, SeatState, SeatHandler};
use smithay::input::keyboard::XkbConfig;
use smithay::output::{Output, PhysicalProperties, Mode, Subpixel, OutputHandler};
use smithay::utils::{Transform, Scale};
use smithay::wayland::socket::ListeningSocketSource;
use wayland_server::{Display, DisplayHandle, Client, backend::{ObjectId, ClientData}};
use wayland_server::protocol::wl_surface::WlSurface;
```

If any import path is wrong, the compiler will tell you. Fix the path, don't
re-research. Common adjustments:
- `smithay::wayland::buffer::BufferHandler` may be at `smithay::wayland::buffer`
- `OutputHandler` may be at `smithay::wayland::output::OutputHandler`
- `with_states` may need a `smithay::utils::Rectangle` import

## run() function pattern

```rust
pub fn run(
    config: Config,
    shutdown: Arc<AtomicBool>,
    status_tx: Option<Sender<usize>>,
) -> anyhow::Result<()> {
    // tracing init
    let _guard = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info".into())
        )
        .try_init();

    let mut display: Display<State> = Display::new()?;
    let display_handle = display.handle();

    // Build state
    let mut state = State {
        display_handle: display_handle.clone(),
        compositor_state: CompositorState::new::<State>(&display_handle),
        shm_state: ShmState::new::<State>(&display_handle, vec![]),
        seat_state: SeatState::<State>::new(),
        seat: { /* create seat */ },
        output: { /* create output */ },
        surfaces: HashMap::new(),
        config: config.clone(),
        status_tx,
    };

    // Socket
    let socket_source = match &config.socket_name {
        Some(name) => ListeningSocketSource::with_name(name)?,
        None => ListeningSocketSource::new_auto()?,
    };
    let socket_name = socket_source.socket_name().to_owned();
    // Print full path
    let xdg = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    println!("wayland-remote listening on: {}/{}", xdg, socket_name);

    // Event loop
    let mut event_loop = EventLoop::<State>::try_new()?;
    let handle = event_loop.handle();

    // Insert socket source
    handle.insert_source(socket_source, |stream, &mut state, _| {
        let client_data = Arc::new(ClientState {
            compositor_state: CompositorClientState::default(),
        }) as Arc<dyn ClientData>;
        state.display_handle.insert_client(stream, client_data)
            .ok();
    })?;

    // SIGINT
    let signals = calloop::signals::Signals::new(&[
        calloop::signals::Signal::SIGINT,
        calloop::signals::Signal::SIGTERM,
    ])?;
    handle.insert_source(signals, |_, _, _| {
        // Can't access shutdown from here easily; use the idle callback instead
        Ok(calloop::PostAction::Continue)
    })?;

    // Dispatch loop
    loop {
        event_loop.dispatch(Some(Duration::from_millis(50)), &mut state)?;
        display.dispatch_clients(&mut state)?;
        display.flush_clients()?;
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
    }

    Ok(())
}
```

NOTE on shutdown: the signals callback can't easily set the shutdown flag
because it doesn't have access to the Arc. Two options:
1. Put the `Arc<AtomicBool>` inside State so the signals callback can set it.
2. Use the idle callback (the 3rd arg to `event_loop.run` / check in the loop).

Option 1 is cleaner — add `pub shutdown: Arc<AtomicBool>` to State, set it in
the signals callback, check it in the loop. Or just use the loop body check
against the external `shutdown` flag (the signals source just needs to exist
to prevent SIGINT from killing the process; the loop checks the flag). But
without the signals source setting the flag, SIGINT won't set it. So: add
shutdown to State, OR use a channel: the signals callback sends a unit on a
channel, the loop checks the channel. Simplest: add `shutdown: Arc<AtomicBool>`
to State.

## Test client (wayland-client 0.31)

```rust
use wayland_client::{Connection, EventQueue, QueueHandle};
use wayland_client::protocol::{wl_compositor::WlCompositor, wl_shm::WlShm, wl_seat::WlSeat, wl_registry::WlRegistry};
use wayland_client::globals::{registry_queue_init, BindError};
use tempfile::tempfile;

pub struct TestClient {
    _conn: Connection,
    _queue: EventQueue<()>,
    // keep objects alive
    _compositor: Option<WlCompositor>,
    _shm: Option<WlShm>,
    _seat: Option<WlSeat>,
}

impl TestClient {
    pub fn connect_and_create_surface(socket_name: &str) -> anyhow::Result<Self> {
        let conn = Connection::connect(socket_name)?;
        let (globals, mut queue) = registry_queue_init::<()>()?; // check API
        // Bind compositor, shm, seat
        let compositor = globals.bind(&queue, 1..=4, ())?;  // check exact API
        let shm = globals.bind(&queue, 1..=1, ())?;
        let _seat = globals.bind(&queue, 1..=7, ())?;

        // Create shm pool + buffer
        let mut file = tempfile()?;
        let pixels = [0xFFAABBCCu32; 64 * 64]; // 64x64 pattern
        use std::io::Write;
        file.write_all(bytemuck::cast_slice(&pixels))?;
        file.seek(SeekFrom::Start(0))?;

        let pool = shm.create_pool(file.as_fd(), (64*64*4) as i32, &queue, ());
        let buffer = pool.create_buffer(0, 64, 64, 256, wayland_client::protocol::wl_shm::Format::Argb8888, &queue, ());

        let surface = compositor.create_surface(&queue, ());
        surface.attach(Some(&buffer), 0, 0);
        surface.commit();

        conn.flush()?;

        // Keep the connection alive; surface persists as long as objects are alive
        Ok(Self { _conn: conn, _queue: queue, _compositor: Some(compositor), _shm: Some(shm), _seat: Some(seat) })
    }
}
```

NOTE: wayland-client 0.31 API may differ slightly from the above. Key types:
`Connection::connect(name)`, `registry_queue_init`, `Globals::bind`. The
`delegate_noop!` macro handles event callbacks for simple clients. Check
compiler errors and adapt. You may need `use wayland_client::delegate_noop!`
and `impl wayland_client::Dispatch<WlRegistry, ()> for () { delegate_noop!(); }`
etc. for the queue's event handler type (here `()`).

If `bytemuck` is not available (not a dep), write the bytes manually:
```rust
let mut buf = Vec::with_capacity(64 * 64 * 4);
for _ in 0..(64*64) {
    buf.extend_from_slice(&0xFFAABBCCu32.to_ne_bytes());
}
file.write_all(&buf)?;
```

## Cargo.toml change

In `crates/server/Cargo.toml`, change:
```toml
calloop.workspace = true
```
to:
```toml
calloop = { workspace = true, features = ["signals"] }
```

No other dependency changes needed.

## Offscreen Rendering with Pixman (Issue 04)

Verified against docs.rs/smithay/0.7.0 — `smithay::backend::renderer::pixman`.

### Key types

- `PixmanRenderer` — the renderer. `PixmanRenderer::new() -> Result<Self, PixmanError>`.
- `pixman::image::bits::Image<'static, 'static>` — an offscreen pixel buffer (from the `pixman` crate 0.2.1, re-exported via smithay). This is what you render into and read back from.
- `PixmanTarget<'a>` — a framebuffer bound to a `PixmanRenderer`. Created by `Bind::bind(&mut renderer, &mut image)`.
- `PixmanTexture` — a texture handle (the `TextureId` for PixmanRenderer). Created by importing a wl_shm buffer.
- `PixmanFrame<'frame, 'buffer>` — the render context returned by `Renderer::render(...)`. Implements the `Frame` trait (check `smithay::backend::renderer::Frame` on docs.rs for exact method names — likely `render_texture` / `render_texture_at`).
- `PixmanMapping` — a downloaded pixel buffer. Created by `ExportMem::copy_framebuffer(...)`.

### Trait impls on PixmanRenderer (from docs.rs)

- `Offscreen<Image<'static, 'static>>` — `create_buffer(format: DrmFourcc, size: Size<i32, BufferCoords>) -> Result<Image<'static, 'static>, PixmanError>`. Creates the offscreen pixel buffer.
- `Bind<Image<'static, 'static>>` — `bind<'a>(&mut self, target: &'a mut Image<'static, 'static>) -> Result<PixmanTarget<'a>, PixmanError>`. Binds the image as a render target.
- `ImportMemWl` — `import_shm_buffer(buffer: &WlBuffer, surface: Option<&SurfaceData>, damage: &[Rectangle<i32, BufferCoords>]) -> Result<PixmanTexture, PixmanError>`. Imports a wl_shm buffer as a texture.
- `ImportAll` (blanket) — `import_buffer(buffer: &WlBuffer, surface: Option<&SurfaceData>, damage: &[Rectangle]) -> Option<Result<PixmanTexture, PixmanError>>`. Tries shm/egl/dma; for pixman without EGL, only shm works.
- `Renderer` — `render<'frame, 'buffer>(&'frame mut self, target: &'frame mut PixmanTarget<'buffer>, output_size: Size<i32, Physical>, dst_transform: Transform) -> Result<PixmanFrame<'frame, 'buffer>, PixmanError>`. Begins a render pass.
- `ExportMem` — `copy_framebuffer(&mut self, target: &PixmanTarget<'_>, region: Rectangle<i32, BufferCoords>, format: DrmFourcc) -> Result<PixmanMapping, PixmanError>` + `map_texture<'a>(&mut self, mapping: &'a PixmanMapping) -> Result<&'a [u8], PixmanError>`. Reads back pixels.

### Render flow (the complete pipeline)

```rust
use smithay::backend::renderer::pixman::PixmanRenderer;
use smithay::backend::renderer::{Renderer, Bind, Offscreen, ExportMem, ImportAll};
use smithay::backend::allocator::Fourcc;
use smithay::utils::{Transform, Rectangle, Size};

// 1. Create renderer
let mut renderer = PixmanRenderer::new()?;

// 2. Create offscreen buffer (the framebuffer)
let format = Fourcc::Argb8888; // BGRA in memory on little-endian
let size: Size<i32, smithay::utils::Buffer> = (width as i32, height as i32).into();
let mut image = renderer.create_buffer(format, size)?;

// 3. Bind as render target
let mut target = renderer.bind(&mut image)?;

// 4. Begin render pass
let output_size: Size<i32, smithay::utils::Physical> = (width as i32, height as i32).into();
let mut frame = renderer.render(&mut target, output_size, Transform::Normal)?;

// 5. Clear (render a full-screen opaque rect, or use frame's clear method)
//    Check PixmanFrame/Frame trait for the exact method — likely:
//    frame.clear([0.0, 0.0, 0.0, 1.0])?;  // or similar

// 6. Import each surface's wl_shm buffer as a texture and render it
//    In commit handler, store the WlBuffer; in render, import it:
//    let texture = renderer.import_buffer(&buffer, Some(surface_data), &damage)?;
//    if let Some(Ok(tex)) = texture {
//        frame.render_texture_at(&tex, (x, y).into(), Transform::Normal, 1.0)?;
//        // or render_texture with explicit src/dst rects — check Frame trait
//    }
//    NOTE: Check the exact Frame trait method signature on docs.rs.
//    The method is likely `render_texture_at(&mut self, texture: &TextureId, pos: Point<i32, Physical>, transform: Transform, alpha: f32) -> Result<Rectangle<i32, Physical>, Error>`.

// 7. Finish the frame (frame is consumed/dropped — check if there's an explicit end() or if Drop finalizes)

// 8. Read back pixels
let region = Rectangle::new((0, 0).into(), (width as i32, height as i32).into());
let mapping = renderer.copy_framebuffer(&target, region, format)?;
let pixels: &[u8] = renderer.map_texture(&mapping)?;
// pixels is BGRA, stride may differ from width*4 — check the image/target size
```

### Getting the surface buffer in commit

```rust
use smithay::wayland::compositor::{with_states, SurfaceAttributes};

// In CompositorHandler::commit:
let buffer_info = with_states(surface, |states| {
    let attrs = states.cached_state.get::<SurfaceAttributes>().current();
    attrs.buffer.as_ref().map(|buf| {
        // BufferAssignment::NewBuffer(WlBuffer) or similar
        // Check the exact type — in 0.7 it's likely:
        // attrs.buffer is Option<BufferAssignment>
        // BufferAssignment::NewBuffer(buffer) => buffer.dimensions()
    })
});
```

NOTE: The exact `SurfaceAttributes` field types may differ in 0.7. Check the compiler. The key is: `attrs.buffer` gives you the committed `WlBuffer`, and `buffer.dimensions()` gives `(u32, u32)`.

### BGRA byte order

`Fourcc::Argb8888` on little-endian = BGRA in memory. This is what pixman produces and what GDI expects. A single pixel with value `0xFFAABBCC` (u32 LE) is stored as bytes `[CC, BB, AA, FF]` = `[B, G, R, A]`. Verify with a test: render `0xFF0000FF` (blue=0xFF, green=0x00, red=0x00, alpha=0xFF) → bytes should be `[FF, 00, 00, FF]` = `[B=255, G=0, R=0, A=255]`.

### Stride

The pixman Image may pad rows. The `PixmanTarget` implements `Texture` trait which has `width()` and `height()` but NOT stride. To get the real stride, either:
1. Use `copy_framebuffer` (ExportMem) which returns a contiguous buffer — the stride is `width * 4` for Argb8888.
2. Access the pixman Image directly (if the `pixman` crate exposes `stride()` on Image — check pixman 0.2.1 docs).

For MVP, use `copy_framebuffer` + `map_texture` for readback — it handles stride for you and returns a contiguous `&[u8]` of `width * height * 4` bytes.

### Cargo.toml: no changes needed

The `renderer_pixman` feature is already enabled on the smithay dependency. The `pixman` crate is a transitive dependency of smithay — you do NOT need to add it directly. The `Image` type comes from smithay's re-export or the `pixman` crate directly; check if smithay re-exports it or if you need `pixman = "0.2.1"` as a direct dep. If the compiler can't find `pixman::image::bits::Image`, add `pixman = "0.2"` to `[workspace.dependencies]` and `crates/server/Cargo.toml`.

### Key risk: Frame trait methods

The exact method names on `PixmanFrame` (the render context) are the main unknown. Check `smithay::backend::renderer::Frame` trait on docs.rs/smithay/0.7.0. Common methods in smithay renderers:
- `render_texture(&mut self, texture, src_rect, dst_rect, transform, alpha) -> Result<(), Error>`
- `render_texture_at(&mut self, texture, pos, transform, alpha) -> Result<Rectangle, Error>`
- `clear(&mut self, color: [f32; 4]) -> Result<(), Error>` (or via a Result-returning method)

If the method names differ, adapt — the pattern is always "begin render → draw textures → implicit finish on drop → readback".

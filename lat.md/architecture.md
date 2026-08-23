# Architecture

System architecture of wayland-remote: a headless Wayland compositor on Linux that streams rendered frames to a native Windows viewer, per [[decisions#Architecture Overview]].

## Crate Map

Three crates in one workspace. `protocol` is the pure wire-format library shared by both sides; `server` is the Smithay compositor plus QUIC endpoint; `viewer` is the Windows client.

## Runtime Split

The compositor runs on a single-threaded calloop event loop; all network I/O runs on a separate tokio runtime.

They communicate through channels: frames out via tokio mpsc with `blocking_send`, input events in via `calloop::channel`. The compositor thread never awaits; the network tasks never touch compositor state directly. See [[decisions#Decision Log#Runtime Split]].

## Rendering Pipeline

Each mapped toplevel renders into its own offscreen target — a GL (EGL) renderer that imports dmabuf when a DRM render node is probed, else the pixman software renderer (wl_shm only) — producing a per-window BGRA frame that is the wire payload.

The compositor loop iterates `WindowManager::mapped_windows()` every tick and runs one render pass per window via `OffscreenRenderer::render_window_surface`, which walks the window's subsurface tree (root first, then each `wl_subsurface` descendant at the position accumulated from its `SubsurfaceCachedState` locations) and draws every committed buffer at its place, clipped to the window rect. The `wp_viewporter` global is served by `ViewporterState` on `State`, but viewport crop/scale is not applied yet — MVP renders buffers at their natural size. The resulting frame is tagged with the window's `window_id` on both `FrameHeader` and `FrameBuffer`; that id is the demux key the viewer uses to route frames to its per-window stores. Committed buffers import as textures (`wl_shm` or dmabuf), and readback yields a top-down BGRA buffer with a real (padded) stride. That buffer — unchanged — is what goes on the wire, so GDI can blit it with zero conversion ([[decisions#Decision Log#BGRA Wire Format]]).

Rendering is gated by a per-window dirty flag (`Window::dirty`, [[crates/server/src/window.rs#Window]]): a window renders only while it may have new pixels. It starts dirty at map; `CompositorHandler::commit` ([[crates/server/src/state.rs#State]]) calls `WindowManager::mark_all_mapped_dirty` ([[crates/server/src/window.rs#WindowManager#mark_all_mapped_dirty]]) on any new-buffer commit (a pointer move no longer re-renders — the cursor is no longer drawn in-frame, so moving it changes no streamed frame). The stream loop in `run` ([[crates/server/src/lib.rs#run]]) consumes the flag via `WindowManager::take_dirty` ([[crates/server/src/window.rs#WindowManager#take_dirty]]) and skips clean windows. A just-mapped window renders exactly once, then stays clean until the next new-buffer commit, so a static window pays no GL import / PBO readback / compress while idle; animating clients commit a new buffer each frame and keep re-rendering. See [[decisions#Decision Log#Per-window change gating]].

The pointer cursor is no longer composited into the streamed frame: `render_window` passes `None` for the cursor to `render_window_surface`, so a frame carries only window content. Instead the server relays cursor state to viewers over the net bridge. `SeatHandler::cursor_image` ([[crates/server/src/state.rs#State]]) stores the `wl_pointer.set_cursor` surface in `State.cursor_surface` (cleared on `Hidden`/`Named` and on surface destroy); on a `Surface` image it resolves a target window (`cursor_window_id`: the pointer-focus window if the pointer is over a tracked toplevel, else the focused window), reads the sprite back through the offscreen renderer (`readback_cursor_sprite`), and sends `NetCommand::CursorShape` (sprite BGRA + hotspot); `Hidden`/`Named` send `NetCommand::CursorHide`. `run`'s input closure ([[crates/server/src/lib.rs#run]]) sends `NetCommand::CursorMove` on each pointer move. The three commands are forwarded to every viewer's control stream (04b-1) and, in issue 04c, drive a native viewer-side cursor. Any cursor readback failure (no renderer, no committed buffer, unimportable buffer) is a logged no-op — never a panic or disconnect. The cursor surface is still excluded from surface tiling (`get_role == "cursor_image"`) so it is not double-drawn.

The renderer is chosen once at startup by `egl::probe` ([[crates/server/src/rendering/egl.rs#probe]]), which globs `/dev/dri/renderD*` and builds a Smithay `GlesRenderer` (GBM→EGLDisplay→EGLContext→GlesRenderer) on the first node where the whole chain succeeds. Because `GlesRenderer` and `PixmanRenderer` satisfy the same trait set (`Renderer + ImportAll + Offscreen<T> + Bind<T> + ExportMem`, with the PBO readback serving the `ExportMem` path), `OffscreenRenderer<R, T>` is generic and the identical render/readback pipeline serves both; `Offscreen { Software, Gl }` ([[crates/server/src/rendering/mod.rs#Offscreen]]) dispatches to whichever was built. When a GL renderer is built, `State` also registers the `zwp_linux_dmabuf` global (`DmabufFeedbackBuilder` with the node's `dev_t` + the display's dmabuf render formats) so EGL/dmabuf clients can attach buffers; on the pixman fallback no dmabuf global is advertised. dmabuf imports are acknowledged in `DmabufHandler::dmabuf_imported` and the texture is created lazily at render time (`import_buffer`→`import_dmabuf`). Note the commit handler must read a buffer's size via `get_dmabuf` for dmabuf buffers — `with_buffer_contents` is SHM-only and returns `NotManaged` otherwise, which silently mapped dmabuf windows at 0×0 (GL FBO bind failure).

The `CompositorHandler::commit` ([[crates/server/src/state.rs#State]]) also fires
the surface's pending present-completion frame callbacks: it drains
`SurfaceAttributes.frame_callbacks` and sends `done` on each with a monotonic
millisecond timestamp from `State.start`. Without this, `wl_surface.frame`-paced
clients (weston-simple-egl, weston-flower) commit one or two frames then stall on
a static image; with it, the scene animates.

## Telemetry

The server keeps lightweight counters on `State` for observability and the test harness: frames streamed, frame bytes, per-second render/readback timing, commits, input events, input-to-commit latency, and errors. See [[architecture#Rendering Pipeline]] and [[architecture#Runtime Split]].

`Telemetry` ([[crates/server/src/state.rs#Telemetry]]) is a `pub` field on `State` ([[crates/server/src/state.rs#State]]); `record_commit` ([[crates/server/src/state.rs#Telemetry#record_commit]]) fires in `CompositorHandler::commit`, `record_input` ([[crates/server/src/state.rs#Telemetry#record_input]]) in `State::inject_input`, and `record_frame` ([[crates/server/src/state.rs#Telemetry#record_frame]])/`record_error` at every `push_frame` ([[crates/server/src/lib.rs#push_frame]]) call site in the compositor loop. `record_frame` also threads the per-frame `render_ns`/`readback_ns` timings (measured in `render_window_surface` and carried on [[crates/server/src/rendering/mod.rs#FrameBuffer]]) into per-second accumulators. A `TelemetrySnapshot` ([[crates/server/src/state.rs#TelemetrySnapshot]]) is published via `snapshot` ([[crates/server/src/state.rs#Telemetry#snapshot]]) roughly once per second — resetting the per-second render/readback accumulators alongside `frames_this_second` — and emitted as a structured `tracing::info!` line from `run` ([[crates/server/src/lib.rs#run]]) that includes `render_ms`/`readback_ms`; `second_start_elapsed` lets the loop poll the per-second window without mutating the counters.

## Input Focus

Injected pointer/keyboard events reach client surfaces via real smithay focus types, not a stub.

The seat's focus types are `WlSurface` (smithay provides `PointerTarget<D> for WlSurface` / `KeyboardTarget<D> for WlSurface` that forward protocol events); a previous `SurfaceFocus` stub was removed. `window_id` travels with each `Message::Input` through the bridge (`CompositorCommand::Input { window_id, event }`) so `inject` ([[crates/server/src/input/mod.rs#inject]]) can resolve the target surface via `WindowManager::surface_for` ([[crates/server/src/window.rs#WindowManager#surface_for]]) and pass `Some((surface, (0,0)))` as the pointer focus to `ptr.motion` — the per-window model treats each window as its own coordinate space with origin (0,0). A `ptr.frame()` follows every pointer event group so real toolkit clients (wl_pointer v5+) that buffer events until a frame actually process them. `SetFocus` also calls `kbd.set_focus` on the resolved surface so keyboard input reaches it.

The server advertises `wl_data_device_manager` (via `DataDeviceState` on `State`). GTK 3.24+ treats this as a hard precondition for binding `wl_seat` — without it, GTK never binds the seat, never gets a keyboard, and fails window activation and popups.

## Compositor Globals

`State` advertises a fixed set of Wayland globals so a range of clients (xdg, legacy, IME-aware) can bind and map.

The global set is: `wl_compositor` + `wl_subcompositor` (`CompositorState`), `wl_shm` (`ShmState`), `xdg_wm_base` (`XdgShellState`), `wp_viewporter` (`ViewporterState`), `wl_data_device_manager` (`DataDeviceState`), `zwp_text_input_v3` (`TextInputManagerState`), legacy `wl_shell` (hand-rolled `WlShellState`), plus `wl_seat` and `wl_output`. `zwp_text_input_v3` needs no handler trait — `TextInputManagerState` only requires `SeatHandler` (already implemented); it advertises the global and tracks focus, but no text flows without a `zwp_input_method_v2` instance (typing works via the keyboard path).

The legacy `wl_shell` global is hand-rolled in `[[crates/server/src/wl_shell.rs]]` because smithay 0.7 ships no legacy shell support. `WindowManager::Window` carries a `ShellKind` (`Xdg(ToplevelSurface)` / `Legacy(WlShellSurface)`) plus a shared `WlSurface`; xdg-only operations (configure, close, activation) match on the variant. Legacy toplevels register on `set_toplevel`, get a size-hint `configure`, and map on first buffer commit — pre-acked since that protocol has no `ack_configure`.

## QUIC Session Model

Each connection is one quinn session: a control stream plus one unidirectional stream per frame, with receiver-side skip-stale.

Control traffic (handshake, input, window events, cursor updates, ping/pong) shares one bidirectional stream; each compressed frame gets its own stream so a lost frame cannot head-of-line-block later frames. Receivers issue STOP_SENDING on stale frame streams — UDP-like drop-oldest semantics without custom loss recovery ([[decisions#Decision Log#Transport]]).

## Viewer

The Windows viewer is a native Win32/GDI client: a background net task drives the QUIC session off the UI thread, and per-window frame stores feed a GDI blit path — the network task never touches GDI.

### Net task and UI thread

A background thread runs a single-threaded tokio runtime that owns the QUIC session; the main thread owns the Win32 message loop and GDI.

They communicate only through tokio channels (input and control commands in) and `PostMessageW` (invalidation out) — the net task never touches GDI or the message loop.

### Per-window frame store

Frame reception swaps each incoming frame into a per-window frame store keyed by `window_id`; the UI child `WndProc` reads the latest frame on paint.

The store map is `Arc<Mutex<HashMap<u64, Arc<FrameStore>>>>`; a store is created lazily on its first frame.

### PostMessageW invalidation

The net task never calls GDI; it posts frame, window-event, RTT, and cursor messages to the controller HWND.

All GDI and cursor work stays on the UI thread.

### Native cursor

The viewer renders the pointer as a native Win32 `HCURSOR` rather than compositing it into the streamed frame.

On `WM_USER_CURSOR_SHAPE` the UI thread builds a 32-bit alpha cursor via `CreateCursor` (AND-mask `NULL`, XOR-mask = the BGRA sprite), destroys the previous `HCURSOR`, and calls `SetCursor`. On `WM_USER_CURSOR_MOVE` it only frees the posted `Box<CursorMoveMsg>`; it does **not** call `SetCursorPos` because the local mouse is the source of truth (the user's `WM_MOUSEMOVE` is forwarded upstream as `PointerMove`), so applying the stale echo would cause a snap-back and a `SetCursorPos → WM_MOUSEMOVE → PointerMove` feedback loop. On `WM_USER_CURSOR_HIDE` (only when the target window is focused) it calls `ShowCursor(0)` and tracks visibility in `Shared.cursor_visible` so the reference count stays balanced. The stored `HCURSOR` is destroyed on `WM_USER_NET_CLOSED` or `WM_CLOSE`.

### GDI blit

Each child window blits its latest frame with GDI `StretchDIBits` as 32bpp `BI_RGB` with a negative `biHeight` (top-down BGRA, matching the pixman readback), stretched to fit the client rect.

A padded server stride is repacked to a tight row before the blit.

### Controller and child windows

The hidden controller HWND owns the message loop and the window manager; every mapped remote toplevel is a child HWND beneath it.

Each toplevel is therefore independently movable, resizable, and closable on the desktop.

### Async orchestration

Frame reception runs on a separate tokio task holding a cloned quinn connection, while the control loop holds the session mutably in a `select!`.

The clone comes from `ViewerSession::connection()`, so `next_frame(&self)` and `send_input(&mut self)` sit on different borrows and do not conflict.

### Drive mode

The `drive` subcommand is a no-GUI scripted client for Windows and Linux: it connects, waits for a window, captures a baseline frame, runs a scripted input sequence, saves the first changed frame as a PNG, and prints a JSON summary.

It waits for a `WindowEvent::Created`, records a `frame_0.png` baseline, sends clicks/keys/waits from the CLI, and captures frames until one differs from the baseline or the `--frames` budget is exhausted.

`drive::run_drive` drives a current-thread tokio runtime like headless mode; the GUI and headless paths are untouched and are still dispatched by `run_display`. The JSON summary is formatted by hand (no `serde`) so the crate has no serialization dependency. See [[crates/viewer/src/display/drive.rs#run_drive]] and [[crates/viewer/src/display/mod.rs#run_display]].

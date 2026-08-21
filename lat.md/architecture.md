# Architecture

System architecture of wayland-remote: a headless Wayland compositor on Linux that streams rendered frames to a native Windows viewer, per [[decisions#Architecture Overview]].

## Crate Map

Three crates in one workspace. `protocol` is the pure wire-format library shared by both sides; `server` is the Smithay compositor plus QUIC endpoint; `viewer` is the Windows client.

## Runtime Split

The compositor runs on a single-threaded calloop event loop; all network I/O runs on a separate tokio runtime.

They communicate through channels: frames out via tokio mpsc with `blocking_send`, input events in via `calloop::channel`. The compositor thread never awaits; the network tasks never touch compositor state directly. See [[decisions#Decision Log#Runtime Split]].

## Rendering Pipeline

Each mapped xdg toplevel renders into its own offscreen pixman target, producing a per-window BGRA frame that is the wire payload.

The compositor loop iterates `WindowManager::mapped_windows()` every tick and runs one render pass per window via `OffscreenRenderer::render_surface`. The resulting frame is tagged with the window's `window_id` on both `FrameHeader` and `FrameBuffer`; that id is the demux key the viewer uses to route frames to its per-window stores. wl_shm buffers import as pixman textures, and readback yields a top-down BGRA buffer with a real (padded) stride. That buffer — unchanged — is what goes on the wire, so GDI can blit it with zero conversion ([[decisions#Decision Log#BGRA Wire Format]]).

## Telemetry

The server keeps lightweight counters on `State` for observability and the test harness: frames streamed, frame bytes, commits, input events, input-to-commit latency, and errors. See [[architecture#Rendering Pipeline]] and [[architecture#Runtime Split]].

`Telemetry` ([[crates/server/src/state.rs#Telemetry]]) is a `pub` field on `State` ([[crates/server/src/state.rs#State]]); `record_commit` ([[crates/server/src/state.rs#Telemetry#record_commit]]) fires in `CompositorHandler::commit`, `record_input` ([[crates/server/src/state.rs#Telemetry#record_input]]) in `State::inject_input`, and `record_frame`/`record_error` at every `push_frame` ([[crates/server/src/lib.rs#push_frame]]) call site in the compositor loop. A `TelemetrySnapshot` ([[crates/server/src/state.rs#TelemetrySnapshot]]) is published via `snapshot` ([[crates/server/src/state.rs#Telemetry#snapshot]]) roughly once per second and emitted as a structured `tracing::info!` line from `run` ([[crates/server/src/lib.rs#run]]); `second_start_elapsed` lets the loop poll the per-second window without mutating the counters.

## Input Focus

Injected pointer/keyboard events reach client surfaces via real smithay focus types, not a stub.

The seat's focus types are `WlSurface` (smithay provides `PointerTarget<D> for WlSurface` / `KeyboardTarget<D> for WlSurface` that forward protocol events); a previous `SurfaceFocus` stub was removed. `window_id` travels with each `Message::Input` through the bridge (`CompositorCommand::Input { window_id, event }`) so `inject` ([[crates/server/src/input/mod.rs#inject]]) can resolve the target surface via `WindowManager::surface_for` ([[crates/server/src/window.rs#WindowManager#surface_for]]) and pass `Some((surface, (0,0)))` as the pointer focus to `ptr.motion` — the per-window model treats each window as its own coordinate space with origin (0,0). A `ptr.frame()` follows every pointer event group so real toolkit clients (wl_pointer v5+) that buffer events until a frame actually process them. `SetFocus` also calls `kbd.set_focus` on the resolved surface so keyboard input reaches it.

## QUIC Session Model

Each connection is one quinn session: a control stream plus one unidirectional stream per frame, with receiver-side skip-stale.

Control traffic (handshake, input, window events, ping/pong) shares one bidirectional stream; each compressed frame gets its own stream so a lost frame cannot head-of-line-block later frames. Receivers issue STOP_SENDING on stale frame streams — UDP-like drop-oldest semantics without custom loss recovery ([[decisions#Decision Log#Transport]]).

## Viewer

The Windows viewer is a native Win32/GDI client: a background net task drives the QUIC session off the UI thread, and per-window frame stores feed a GDI blit path — the network task never touches GDI.

### Net task and UI thread

A background thread runs a single-threaded tokio runtime that owns the QUIC session; the main thread owns the Win32 message loop and GDI.

They communicate only through tokio channels (input and control commands in) and `PostMessageW` (invalidation out) — the net task never touches GDI or the message loop.

### Per-window frame store

Frame reception swaps each incoming frame into a per-window frame store keyed by `window_id`; the UI child `WndProc` reads the latest frame on paint.

The store map is `Arc<Mutex<HashMap<u64, Arc<FrameStore>>>>`; a store is created lazily on its first frame.

### PostMessageW invalidation

The net task never calls GDI; it posts `WM_USER_FRAME`, `WM_USER_WIN_EVENT`, and `WM_USER_RTT` to the controller HWND, which invalidates or resizes the matching child windows.

All GDI work stays on the UI thread.

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

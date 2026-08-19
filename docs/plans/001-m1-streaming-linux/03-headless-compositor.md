# Issue 03 — Headless Smithay compositor

## Objective

(PRD Step 1) A running Smithay 0.7 compositor: accepts Wayland clients on its own socket, tracks surfaces, seat, and outputs in state, driven by a calloop event loop. No rendering yet — verified by log lines and an in-repo test client.

## Files

| File | Change |
|---|---|
| `crates/server/src/state.rs` | `State` struct: `DisplayHandle`, `DmArena`-free minimal fields — surfaces map, `KeyboardHandle`, `PointerHandle`, `OutputHandle`-less (headless), config (resolution, socket name) |
| `crates/server/src/handlers/mod.rs` + `compositor.rs`, `shm.rs`, `seat.rs` | `delegate_compositor!`, `delegate_shm!`, `delegate_seat!` + handler impls |
| `crates/server/src/main.rs` | arg parsing (width/height defaults 1280x720, socket name override, log filter), socket setup, calloop `DefaultLoop`, dispatch |
| `crates/server/src/lib.rs` | `run(config) -> anyhow::Result<()>` so integration tests can drive it |
| `crates/server/tests/common/mod.rs` | Test client harness: connect via `wayland-client` to a spawned server, registry bind, create wl_shm pool + surface, commit |
| `crates/server/tests/compositor.rs` | Tests below |

## Implementation notes

- Socket: default `$XDG_RUNTIME_DIR/wayland-remote-<pid>.sock`; print full path at startup (PRD §5's SSH story depends on this being printable/greppable). Smithay's `Display::add_socket_auto` is the fallback if the env var is missing.
- Smithay 0.7 API surface for this issue: `wayland_server::Display`, `DisplayHandle`, `Backend::listen_fd`/socket registration via `add_socket`, `delegate_*!` macros, `smithay::wayland::compositor::{CompositorHandler, with_surface_tree_downward}`, `smithay::wayland::seat::{Seat, SeatHandler, KeyboardHandle, PointerHandle, Capability}`. Follow the **Smallvil** example (github.com/Smithay/smallvil) for the 0.7-era handler wiring — it is the minimal reference compositor on the same API line.
- Surface tracking: on `commit` (`CompositorHandler::commit`), record surface buffer dimensions in `State` keyed by `WlSurface` id; log at debug level. This map is what issue 04 renders and issue 05 streams.
- Seat: create one seat named "wayland-remote" with keyboard + pointer capabilities. The `KeyboardHandle` needs a keymap loaded — use smithay's xkb helper to load "default" keymap at init (this is what issue 07's injection will drive).
- The event loop: pure calloop. `std::os::unix::net` listener from smithay's socket API, dispatch display events, no threads yet.
- Graceful shutdown: SIGINT via calloop signal (or a simple channel) → remove socket file, clean exit. Integration tests spawn the server as a child process or in-process thread; prefer in-process via `lib.rs::run` with a cancel token (`Arc<AtomicBool>`).

## Steps

1. `state.rs` with `State` + `smithay::delegate_compositor!`-style boilerplate for the three globals (compositor, shm, seat).
2. `main.rs` wiring: display, socket, loop, tracing-subscriber with `RUST_LOG` env filter.
3. Test harness `common/mod.rs`: `TestClient::connect(socket_path)` using `wayland-client` 0.31 + `wayland-protocols` (for `wl_shm` via core; no xdg yet). Creates a 64x64 pool, fills a recognizable pattern (see issue 04), attaches + commits.
4. Write tests (see Verification); wire `@lat:` refs.

## Verification

- Test `client_connects_and_creates_surface`: server up → test client connects, binds wl_compositor/wl_shm/wl_seat, creates + commits a surface; server state records the surface (expose a `State::surface_count()` snapshot via a test-only mpsc back-channel — keep production code clean by having `run` accept an `Option<Sender<StatusEvent>>`).
- Test `multiple_clients_supported`: two concurrent clients, two surfaces.
- Test `client_disconnect_cleans_up`: drop client → surface removed from state (post-dispatch).
- Manual: `WAYLAND_DISPLAY=<path> WAYLAND_DEBUG=1 weston-simple-egl` or `wayland-info` connects and lists globals (needs apt packages from setup).
- Clippy/fmt clean; `lat check` green with new test-spec sections.

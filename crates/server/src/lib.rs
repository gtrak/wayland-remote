//! Server library for wayland-remote.
//!
//! Shared logic lives in a library so integration tests (see `tests/`) can
//! import it. The headless Wayland compositor lands in plan 001 issue 03;
//! the QUIC frame server lands in issue 05.

pub mod bridge;
pub mod input;
pub mod net;
pub mod rendering;
pub mod state;
pub mod window;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use calloop::EventLoop;
use calloop::signals::{Signal, Signals};
use smithay::wayland::compositor::CompositorClientState;
use smithay::wayland::socket::ListeningSocketSource;
use wayland_server::Display;
use wayland_server::backend::ClientData;

use crate::bridge::{CompositorCommand, NetCommand, channels};
use crate::net::cert::ServerCert;
use crate::net::{NetSettings, run_server};
use crate::rendering::{FrameBuffer, OffscreenRenderer, RenderRequest};
use crate::state::{ClientState, Config, State};

/// Returns the crate version.
#[must_use]
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Runs the headless Wayland compositor.
///
/// Binds a Wayland socket, wires up the compositor/shm/seat/output globals,
/// and dispatches client events until `shutdown` is set (SIGINT/SIGTERM also
/// set it). When `status_tx` is present, the current tracked-surface count is
/// sent on it after every change — the test back-channel.
pub fn run(
    config: Config,
    shutdown: Arc<AtomicBool>,
    status_tx: Option<Sender<usize>>,
    render_rx: Option<Receiver<RenderRequest>>,
) -> anyhow::Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .try_init();

    let mut display: Display<State> = Display::new()?;
    let display_handle = display.handle();

    let mut state = State::new(
        display_handle,
        config,
        status_tx,
        render_rx,
        shutdown.clone(),
    )?;

    // Initialize the offscreen renderer after display setup.
    state.renderer = Some(OffscreenRenderer::new(
        state.config.width,
        state.config.height,
    )?);

    let socket_source = match &state.config.socket_name {
        Some(name) => ListeningSocketSource::with_name(name)?,
        None => ListeningSocketSource::new_auto()?,
    };
    let socket_name = socket_source.socket_name().to_string_lossy().into_owned();
    let xdg = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".to_owned());
    println!("wayland-remote listening on: {}/{}", xdg, socket_name);
    tracing::info!(socket = %socket_name, "wayland-remote headless compositor ready");

    let mut event_loop = EventLoop::<State>::try_new()?;
    let handle = event_loop.handle();

    handle.insert_source(socket_source, |stream, _, state| {
        let client_data = Arc::new(ClientState {
            compositor_state: CompositorClientState::default(),
        }) as Arc<dyn ClientData>;
        if let Err(err) = state.display_handle.insert_client(stream, client_data) {
            tracing::warn!(?err, "failed to insert client");
        }
    })?;

    let signals = Signals::new(&[Signal::SIGINT, Signal::SIGTERM])?;
    handle.insert_source(signals, |_, _, state| {
        tracing::info!("signal received, shutting down");
        state.shutdown.store(true, Ordering::SeqCst);
    })?;

    // Optional QUIC frame server: all network I/O runs on a dedicated tokio
    // runtime; frames cross to it through `frame_tx`, commands come back over
    // the calloop channel inserted below (the compositor thread never awaits).
    let mut runtime: Option<tokio::runtime::Runtime> = None;
    let mut frame_tx: Option<tokio::sync::mpsc::UnboundedSender<NetCommand>> = None;
    let frame_counter = Arc::new(AtomicU64::new(0));
    if let Some(listen) = state.config.listen {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .enable_all()
            .build()?;
        let cert = ServerCert::load_or_generate()?;
        let settings = NetSettings {
            listen,
            compression: state.config.compression,
            cert,
            width: state.config.width,
            height: state.config.height,
        };
        let (net_bridge, comp_bridge) = channels();
        rt.spawn(async move {
            if let Err(err) = run_server(&settings, net_bridge).await {
                tracing::error!(?err, "QUIC frame server failed");
            }
        });
        // The calloop channel's receiver is the event source; the closure
        // owns its own copies of the bridge sender and counter so the loop
        // body below can keep using `frame_tx`/`frame_counter` freely.
        let bridge_tx = comp_bridge.frame_tx.clone();
        let counter = frame_counter.clone();
        let insert = handle.insert_source(comp_bridge.input_rx, move |event, _, state| {
            if let calloop::channel::Event::Msg(cmd) = event {
                match cmd {
                    CompositorCommand::Input { window_id, event } => {
                        let serial = state.input_router.next_serial();
                        let time = state.input_router.now_ms();
                        state.inject_input(window_id, event, serial, time);
                    }
                    CompositorCommand::SetFocus { window_id } => {
                        state.window_manager.set_focus(window_id);
                        // Also set keyboard focus on the seat so key events
                        // reach the surface.
                        if let Some(surface) =
                            state.window_manager.surface_for(window_id).cloned()
                            && let Some(kbd) = state.seat.get_keyboard()
                        {
                            let serial = state.input_router.next_serial();
                            kbd.set_focus(state, Some(surface), serial);
                        }
                    }
                    CompositorCommand::ConfigureWindow {
                        window_id,
                        width,
                        height,
                    } => {
                        state
                            .window_manager
                            .configure_window(window_id, width, height);
                    }
                    CompositorCommand::CloseWindow { window_id } => {
                        state.window_manager.close_window(window_id);
                    }
                    CompositorCommand::RenderRequest(req) => match req {
                        RenderRequest::Render { reply } => {
                            match state.render_frame() {
                                Ok(frame) => {
                                    let _ = reply.send(frame.clone());
                                    state.telemetry.record_frame(frame.data.len());
                                    push_frame(&bridge_tx, &counter, frame);
                                }
                                Err(err) => {
                                    state.telemetry.record_error();
                                    tracing::warn!(?err, "render request failed");
                                }
                            }
                        }
                        RenderRequest::RenderWindow { window_id, reply } => {
                            match state.render_window(window_id) {
                                Ok(frame) => {
                                    let _ = reply.send(frame);
                                }
                                Err(err) => {
                                    state.telemetry.record_error();
                                    tracing::warn!(?err, window_id, "render window request failed");
                                }
                            }
                        }
                    },
                }
            }
        });
        // `InsertError` holds the `!Sync` std-mpsc receiver inside the
        // channel source, so it cannot be wrapped in `anyhow::Error`.
        if let Err(err) = insert {
            return Err(anyhow::anyhow!(
                "failed to insert network input channel: {err}"
            ));
        }
        frame_tx = Some(comp_bridge.frame_tx);
        runtime = Some(rt);
        tracing::info!(listen = %listen, "QUIC frame server enabled");
    }

    loop {
        event_loop.dispatch(Some(Duration::from_millis(50)), &mut state)?;
        display.dispatch_clients(&mut state)?;
        display.flush_clients()?;

        // Relay window lifecycle events (Created/Destroyed/…) to viewers on
        // their control streams.
        if let Some(tx) = &frame_tx {
            let events = state.window_manager.drain_events();
            if !events.is_empty() && tx.send(NetCommand::WindowEvents(events)).is_err() {
                tracing::debug!("network channel closed; window events dropped");
            }
        }

        // Drain any render requests from the test back-channel.
        let requests = state.render_rx.as_ref().map(|rx| {
            let mut reqs = Vec::new();
            while let Ok(req) = rx.try_recv() {
                reqs.push(req);
            }
            reqs
        });
        if let Some(reqs) = requests {
            for req in reqs {
                match req {
                    RenderRequest::Render { reply } => {
                        match state.render_frame() {
                            Ok(frame) => {
                                let _ = reply.send(frame.clone());
                                if let Some(tx) = &frame_tx {
                                    state.telemetry.record_frame(frame.data.len());
                                    push_frame(tx, &frame_counter, frame);
                                }
                            }
                            Err(err) => {
                                state.telemetry.record_error();
                                tracing::warn!(?err, "render request failed");
                            }
                        }
                    }
                    RenderRequest::RenderWindow { window_id, reply } => {
                        match state.render_window(window_id) {
                            Ok(frame) => {
                                let _ = reply.send(frame);
                            }
                            Err(err) => {
                                state.telemetry.record_error();
                                tracing::warn!(?err, window_id, "render window request failed");
                            }
                        }
                    }
                }
            }
        }

        // `--snapshot`: once a client has committed a surface, render a single
        // frame, write it as a PNG, and request shutdown.
        let snapshot = state.config.snapshot.clone();
        if let Some(path) = snapshot.filter(|_| !state.snapshot_done && state.surface_count() > 0) {
            match state.render_frame() {
                Ok(frame) => {
                    if let Err(err) = frame.write_png(&path) {
                        tracing::error!(?err, "failed to write snapshot");
                    }
                    state.snapshot_done = true;
                    state.shutdown.store(true, Ordering::SeqCst);
                }
                Err(err) => tracing::warn!(?err, "snapshot render failed"),
            }
        }

        // Stream a fresh frame per mapped window to connected viewers, each
        // tagged with its window id. The sender is cloned so `state` can be
        // borrowed mutably by `render_window` while the sender is held.
        if let Some(tx) = frame_tx.clone() {
            for window_id in state.window_manager.mapped_windows() {
                match state.render_window(window_id) {
                    Ok(frame) => {
                        state.telemetry.record_frame(frame.data.len());
                        push_frame(&tx, &frame_counter, frame);
                    }
                    Err(err) => {
                        state.telemetry.record_error();
                        tracing::debug!(?err, window_id, "per-window render skipped");
                    }
                }
            }
        }

        // Emit a telemetry snapshot roughly once per second.
        if state.telemetry.second_start_elapsed() >= Duration::from_secs(1) {
            let snap = state.telemetry.snapshot();
            tracing::info!(
                fps = snap.frames_per_sec,
                frames = snap.frames_total,
                bytes = snap.frame_bytes_total,
                commits = snap.commits_total,
                inputs = snap.input_events_total,
                in2commit = ?snap.last_input_to_commit_ms,
                errors = snap.errors_total,
                "telemetry"
            );
        }

        if shutdown.load(Ordering::Relaxed) {
            break;
        }
    }

    // Stop the QUIC server: tell the frame pump to exit, then let the tokio
    // runtime drain its tasks (bounded, so a stuck session cannot hold us up).
    if let Some(tx) = &frame_tx {
        let _ = tx.send(NetCommand::Shutdown);
    }
    if let Some(rt) = runtime {
        rt.shutdown_timeout(Duration::from_secs(5));
    }

    Ok(())
}

/// Assign the next frame sequence number and push `frame` to the network
/// side. A send error means the network side is gone; the frame is dropped.
fn push_frame(
    frame_tx: &tokio::sync::mpsc::UnboundedSender<NetCommand>,
    frame_counter: &Arc<AtomicU64>,
    frame: FrameBuffer,
) {
    let frame_id = frame_counter.fetch_add(1, Ordering::SeqCst) + 1;
    if frame_tx
        .send(NetCommand::Frame { frame, frame_id })
        .is_err()
    {
        tracing::debug!("frame channel closed; frame streaming stopped");
    }
}

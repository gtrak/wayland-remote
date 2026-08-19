//! Server library for wayland-remote.
//!
//! Shared logic lives in a library so integration tests (see `tests/`) can
//! import it. The headless Wayland compositor lands in plan 001 issue 03;
//! the QUIC frame server lands in issue 05.

pub mod rendering;
pub mod state;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;

use calloop::EventLoop;
use calloop::signals::{Signal, Signals};
use smithay::wayland::compositor::CompositorClientState;
use smithay::wayland::socket::ListeningSocketSource;
use wayland_server::Display;
use wayland_server::backend::ClientData;

use crate::rendering::{OffscreenRenderer, RenderRequest};
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

    loop {
        event_loop.dispatch(Some(Duration::from_millis(50)), &mut state)?;
        display.dispatch_clients(&mut state)?;
        display.flush_clients()?;

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
                let RenderRequest::Render { reply } = req;
                match state.render_frame() {
                    Ok(frame) => {
                        let _ = reply.send(frame);
                    }
                    Err(err) => tracing::warn!(?err, "render request failed"),
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

        if shutdown.load(Ordering::Relaxed) {
            break;
        }
    }

    Ok(())
}

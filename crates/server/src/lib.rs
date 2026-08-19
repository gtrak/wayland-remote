//! Server library for wayland-remote.
//!
//! Shared logic lives in a library so integration tests (see `tests/`) can
//! import it. The headless Wayland compositor lands in plan 001 issue 03;
//! the QUIC frame server lands in issue 05.

pub mod state;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::Duration;

use calloop::EventLoop;
use calloop::signals::{Signal, Signals};
use smithay::wayland::compositor::CompositorClientState;
use smithay::wayland::socket::ListeningSocketSource;
use wayland_server::Display;
use wayland_server::backend::ClientData;

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
) -> anyhow::Result<()> {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .try_init();

    let mut display: Display<State> = Display::new()?;
    let display_handle = display.handle();

    let mut state = State::new(display_handle, config, status_tx, shutdown.clone())?;

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
        if shutdown.load(Ordering::Relaxed) {
            break;
        }
    }

    Ok(())
}

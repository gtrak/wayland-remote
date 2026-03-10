//! Wayland Remote Server
//! 
//! Headless Wayland compositor that streams frames to Windows viewers.
//! 
//! This module implements the core compositor using Smithay 0.7.0,
//! following the Smallvil pattern for minimal viable implementation.

use anyhow::Result;
use smithay::reexports::{
    calloop::EventLoop,
    wayland_server::Display,
};
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

mod state;
use state::ServerState;

fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)?;
    
    info!("Wayland Remote Server starting...");
    info!("Server version: {}", env!("CARGO_PKG_VERSION"));
    
    // Create calloop event loop
    // This is the main event dispatcher for Wayland protocol events
    let mut event_loop: EventLoop<ServerState> = EventLoop::try_new()?;
    info!("Event loop initialized");
    
    // Create Wayland display
    // This manages the Wayland protocol state and client connections
    let display: Display<ServerState> = Display::new()?;
    info!("Wayland display created");
    
    // Initialize server state
    // This creates:
    // - CompositorState (advertises wl_compositor global)
    // - ListeningSocketSource (accepts client connections)
    // - Integrates both into the event loop
    let mut state = ServerState::new(&mut event_loop, display);
    
    // Print socket information for clients
    let socket_path = state.socket_path();
    let socket_name = state.socket_name.to_string_lossy();
    
    info!("Wayland socket created: {}", socket_name);
    info!("Full socket path: {}", socket_path.to_string_lossy());
    info!("Set WAYLAND_DISPLAY={} for clients to connect", socket_name);
    
    // Run the event loop
    // This blocks and dispatches Wayland events forever
    // The closure is called on every iteration (for cleanup if needed)
    info!("Entering event loop - ready to accept client connections");
    event_loop.run(None, &mut state, |_| {})?;
    
    info!("Event loop exited");
    Ok(())
}

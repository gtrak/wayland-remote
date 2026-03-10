//! Wayland Remote Viewer
//! 
//! Windows application that connects to the Wayland remote server
//! and displays remote application windows.

#![cfg(windows)]

use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)?;
    
    info!("Wayland Remote Viewer starting...");
    info!("Viewer version: {}", env!("CARGO_PKG_VERSION"));
    
    // Placeholder for actual viewer implementation
    // Phase 4: TCP client connection
    // Phase 5: Window creation with winit
    // Phase 6: Multi-window management
    // Phase 7: XDG shell window states
    // Phase 8: Input capture and transmission
    
    info!("Viewer initialization complete.");
    info!("This is a placeholder - actual window implementation in Phase 5.");
    info!("Press Ctrl+C to exit.");
    
    // Keep running (placeholder until winit event loop)
    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

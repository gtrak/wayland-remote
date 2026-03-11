//! Wayland Remote Viewer
//! 
//! Windows application that connects to the Wayland remote server
//! and displays remote application windows.

use anyhow::Result;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[cfg(windows)]
use wayland_remote_viewer::app;

fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)?;
    
    info!("Wayland Remote Viewer starting...");
    info!("Viewer version: {}", env!("CARGO_PKG_VERSION"));
    
    #[cfg(windows)]
    {
        // Default server address
        let server_address = "127.0.0.1:8080";
        
        info!("Connecting to server at {}...", server_address);
        
        // Run the viewer application
        app::run(server_address)?;
    }
    
    #[cfg(not(windows))]
    {
        info!("This application is designed for Windows only.");
        info!("Please run on a Windows system.");
    }
    
    Ok(())
}

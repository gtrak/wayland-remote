//! Wayland Remote Viewer
//! 
//! Windows application that connects to the Wayland remote server
//! and displays remote application windows.

use anyhow::{Context, Result};
use std::env;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

#[cfg(windows)]
use wayland_remote_viewer::app;

/// Default server address if not specified via CLI
const DEFAULT_SERVER: &str = "127.0.0.1:8080";

/// Parse command line arguments
/// 
/// # Arguments
/// * `args` - Command line arguments (including program name)
/// 
/// # Returns
/// Server address string, or DEFAULT_SERVER if not specified
fn parse_args(args: env::Args) -> String {
    let mut server_address = DEFAULT_SERVER.to_string();
    
    let args_vec: Vec<String> = args.collect();
    
    // Skip program name
    if args_vec.len() > 1 {
        let mut i = 1;
        while i < args_vec.len() {
            match args_vec[i].as_str() {
                "--server" | "-s" => {
                    if i + 1 < args_vec.len() {
                        server_address = args_vec[i + 1].clone();
                        i += 2;
                    } else {
                        eprintln!("Error: --server requires an argument");
                        std::process::exit(1);
                    }
                }
                "--help" | "-h" => {
                    print_help();
                    i += 1;
                    std::process::exit(0);
                }
                "--version" | "-v" => {
                    println!("Wayland Remote Viewer {}", env!("CARGO_PKG_VERSION"));
                    i += 1;
                    std::process::exit(0);
                }
                _ => {
                    eprintln!("Unknown argument: {}", args_vec[i]);
                    eprintln!("Use --help for usage information");
                    i += 1;
                    std::process::exit(1);
                }
            }
        }
    }
    
    server_address
}

/// Print help message
fn print_help() {
    println!("Wayland Remote Viewer {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("Usage: wayland-remote-viewer [OPTIONS]");
    println!();
    println!("Options:");
    println!("  --server, -s <host:port>  Server address to connect to");
    println!("                            (default: {})", DEFAULT_SERVER);
    println!("  --help, -h                Show this help message");
    println!("  --version, -v             Show version information");
    println!();
    println!("Examples:");
    println!("  wayland-remote-viewer                    # Connect to default server");
    println!("  wayland-remote-viewer --server 192.168.1.100:8080");
    println!("  wayland-remote-viewer -s localhost:9000");
}

fn main() -> Result<()> {
    // Initialize logging
    let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();
    
    tracing::subscriber::set_global_default(subscriber)
        .context("Failed to set tracing subscriber")?;
    
    info!("Wayland Remote Viewer starting...");
    info!("Viewer version: {}", env!("CARGO_PKG_VERSION"));
    
    #[cfg(windows)]
    {
        // Parse command line arguments
        let server_address = parse_args(env::args());
        
        info!("Connecting to server at {}...", server_address);
        
        // Run the viewer application
        app::run(server_address)
            .context("Failed to run viewer application")?;
    }
    
    #[cfg(not(windows))]
    {
        info!("This application is designed for Windows only.");
        info!("Please run on a Windows system.");
    }
    
    Ok(())
}

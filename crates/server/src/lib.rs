//! Wayland Remote Server Library
//! 
//! Library exports for testing and modular development.
//! The server can be used as both a binary and a library crate.

#![warn(missing_docs)]
#![warn(rust_2018_idioms)]

/// Handler modules for Wayland protocol globals
pub mod handlers;

/// Rendering module for headless/offscreen rendering
pub mod rendering;

use tracing::debug;
/// Server configuration and state
#[derive(Debug)]
pub struct ServerConfig {
    /// Server version
    pub version: String,
    /// Server name
    pub name: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            name: "wayland-remote-server".to_string(),
        }
    }
}

impl ServerConfig {
    /// Create a new server configuration
    pub fn new() -> Self {
        Self::default()
    }
    
    /// Log configuration details
    pub fn log_config(&self) {
        debug!("Server config: name={}, version={}", self.name, self.version);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_config_defaults() {
        let config = ServerConfig::default();
        assert_eq!(config.name, "wayland-remote-server");
        assert!(!config.version.is_empty());
    }
    
    #[test]
    fn test_config_new() {
        let config = ServerConfig::new();
        assert_eq!(config.name, "wayland-remote-server");
    }
}

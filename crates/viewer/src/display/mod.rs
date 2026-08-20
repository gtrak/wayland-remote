//! Platform display layer.

#[cfg(windows)]
pub mod win;

mod drive;
mod headless;

pub use drive::{DriveAction, DriveConfig, run_drive};
pub use headless::run_headless;

#[cfg(windows)]
pub fn run_display(
    addr: std::net::SocketAddr,
    fingerprint: Option<[u8; 32]>,
    insecure: bool,
    headless: Option<u64>,
) -> anyhow::Result<()> {
    if let Some(secs) = headless {
        return run_headless(addr, fingerprint, insecure, secs);
    }
    win::run(addr, fingerprint, insecure)
}

#[cfg(not(windows))]
pub fn run_display(
    addr: std::net::SocketAddr,
    fingerprint: Option<[u8; 32]>,
    insecure: bool,
    headless: Option<u64>,
) -> anyhow::Result<()> {
    if let Some(secs) = headless {
        return run_headless(addr, fingerprint, insecure, secs);
    }
    anyhow::bail!(
        "This binary was built without Windows display support. Use --headless <secs> for testing on Linux."
    )
}

//! Entry point for the wayland-remote viewer.
//!
//! Issue 01 scaffold: prints the binary name and version, then exits.
//! The QUIC client, Win32 window, and GDI blit loop land in later issues.

fn main() {
    let version = wayland_remote_viewer::version();
    let name = env!("CARGO_PKG_NAME");
    println!("{name} {version}");
    tracing::info!(
        version,
        "wayland-remote-viewer scaffold (streaming arrives in issues 04-05)"
    );
}

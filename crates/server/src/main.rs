//! Entry point for the wayland-remote server.
//!
//! Issue 01 scaffold: prints the binary name and version, then exits.
//! Argument parsing, the smithay compositor, and the QUIC streaming server
//! land in plan 001 issue 03.

fn main() {
    let version = wayland_remote_server::version();
    let name = env!("CARGO_PKG_NAME");
    println!("{name} {version}");
    tracing::info!(
        version,
        "wayland-remote-server scaffold (compositor arrives in issue 03)"
    );
}

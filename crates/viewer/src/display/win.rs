//! Win32 display layer (only compiled on Windows).
//
// The full window/message-loop/GDI implementation is deferred until the
// session + input logic is tested. For now this is a placeholder.

pub fn run(
    _addr: std::net::SocketAddr,
    _fingerprint: Option<[u8; 32]>,
    _insecure: bool,
) -> anyhow::Result<()> {
    unimplemented!("Win32 display not yet implemented")
}

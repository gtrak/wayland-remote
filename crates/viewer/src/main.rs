//! Entry point for the wayland-remote viewer.

use std::net::SocketAddr;

fn main() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .try_init();

    let opts = parse_args();
    if let Err(e) = wayland_remote_viewer::display::run_display(
        opts.addr,
        opts.fingerprint,
        opts.insecure,
        opts.headless,
    ) {
        eprintln!("viewer: {e:?}");
        std::process::exit(1);
    }
}

struct Options {
    addr: SocketAddr,
    fingerprint: Option<[u8; 32]>,
    insecure: bool,
    headless: Option<u64>,
}

fn parse_args() -> Options {
    let mut addr: Option<SocketAddr> = None;
    let mut fingerprint: Option<[u8; 32]> = None;
    let mut insecure = false;
    let mut headless: Option<u64> = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--addr" => {
                addr = Some(
                    args.next()
                        .unwrap_or_else(|| fail("missing --addr"))
                        .parse()
                        .unwrap_or_else(|_| fail("invalid addr")),
                );
            }
            "--fingerprint" => {
                let hex = args.next().unwrap_or_else(|| fail("missing --fingerprint"));
                let bytes: Vec<u8> = (0..hex.len())
                    .step_by(2)
                    .map(|i| {
                        u8::from_str_radix(&hex[i..i + 2], 16)
                            .unwrap_or_else(|_| fail("invalid hex"))
                    })
                    .collect();
                fingerprint = Some(
                    bytes
                        .try_into()
                        .unwrap_or_else(|_| fail("fingerprint must be 64 hex chars")),
                );
            }
            "--insecure" => insecure = true,
            "--headless" => {
                headless = Some(
                    args.next()
                        .unwrap_or_else(|| fail("missing --headless secs"))
                        .parse()
                        .unwrap_or_else(|_| fail("invalid secs")),
                );
            }
            "--help" | "-h" => {
                println!(
                    "usage: wayland-remote-viewer --addr ip:port [--fingerprint hex | --insecure] [--headless secs]"
                );
                std::process::exit(0);
            }
            other => fail(&format!("unknown arg: {other}")),
        }
    }
    let addr = addr.unwrap_or_else(|| fail("--addr is required"));
    if !insecure && fingerprint.is_none() {
        fail("--fingerprint <hex> or --insecure is required");
    }
    Options {
        addr,
        fingerprint,
        insecure,
        headless,
    }
}

fn fail(msg: &str) -> ! {
    eprintln!("viewer: {msg}");
    eprintln!(
        "usage: wayland-remote-viewer --addr ip:port [--fingerprint hex | --insecure] [--headless secs]"
    );
    std::process::exit(2);
}

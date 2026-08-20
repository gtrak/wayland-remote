//! Entry point for the wayland-remote viewer.

use std::net::SocketAddr;
use std::path::PathBuf;

use wayland_remote_viewer::display::{DriveAction, DriveConfig, run_drive};

fn main() {
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .try_init();

    let argv: Vec<String> = std::env::args().collect();

    // No-GUI scripted client mode: `wayland-remote-viewer drive --addr ... ...`
    if argv.get(1).map(String::as_str) == Some("drive") {
        let config = match parse_drive_args(&argv[2..]) {
            Ok(c) => c,
            Err(msg) => fail(&msg),
        };
        if let Err(e) = run_drive(config) {
            eprintln!("viewer: {e:?}");
            std::process::exit(1);
        }
        return;
    }

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

/// Parse the args for the `drive` subcommand (everything after `drive`).
fn parse_drive_args(args: &[String]) -> Result<DriveConfig, String> {
    let mut addr: Option<SocketAddr> = None;
    let mut fingerprint: Option<[u8; 32]> = None;
    let mut insecure = false;
    let mut max_frames: usize = 30;
    let mut out_dir = PathBuf::from(".");
    let mut actions: Vec<DriveAction> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let arg = &args[i];
        match arg.as_str() {
            "--addr" => {
                i += 1;
                let v = args.get(i).ok_or_else(|| "missing --addr".to_string())?;
                addr = Some(v.parse().map_err(|_| format!("invalid addr: {v}"))?);
            }
            "--fingerprint" => {
                i += 1;
                let hex = args
                    .get(i)
                    .ok_or_else(|| "missing --fingerprint".to_string())?;
                let bytes: Vec<u8> = (0..hex.len())
                    .step_by(2)
                    .map(|j| {
                        u8::from_str_radix(&hex[j..j + 2], 16)
                            .map_err(|_| format!("invalid hex: {hex}"))
                    })
                    .collect::<Result<_, _>>()?;
                fingerprint = Some(
                    bytes
                        .try_into()
                        .map_err(|_| "fingerprint must be 64 hex chars".to_string())?,
                );
            }
            "--insecure" => insecure = true,
            "--click" => {
                i += 1;
                let spec = args
                    .get(i)
                    .ok_or_else(|| "missing --click value".to_string())?;
                let mut parts = spec.split(',');
                let x: f64 = parts
                    .next()
                    .ok_or_else(|| format!("bad --click: {spec}"))?
                    .parse()
                    .map_err(|_| format!("bad --click x: {spec}"))?;
                let y: f64 = parts
                    .next()
                    .ok_or_else(|| format!("bad --click: {spec}"))?
                    .parse()
                    .map_err(|_| format!("bad --click y: {spec}"))?;
                // Default button 272 = BTN_LEFT; override with a third field.
                let button = match parts.next() {
                    Some(b) if !b.is_empty() => b
                        .parse::<u32>()
                        .map_err(|_| format!("bad --click button: {spec}"))?,
                    _ => 272,
                };
                actions.push(DriveAction::Click { x, y, button });
            }
            "--key" => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or_else(|| "missing --key value".to_string())?;
                let scancode: u16 = v.parse().map_err(|_| format!("bad --key scancode: {v}"))?;
                actions.push(DriveAction::KeyPress { scancode });
            }
            "--wait" => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or_else(|| "missing --wait value".to_string())?;
                let ms: u64 = v.parse().map_err(|_| format!("bad --wait ms: {v}"))?;
                actions.push(DriveAction::Wait { ms });
            }
            "--frames" => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or_else(|| "missing --frames value".to_string())?;
                max_frames = v.parse().map_err(|_| format!("bad --frames: {v}"))?;
            }
            "--out" => {
                i += 1;
                let v = args
                    .get(i)
                    .ok_or_else(|| "missing --out value".to_string())?;
                out_dir = PathBuf::from(v);
            }
            "--help" | "-h" => {
                println!(
                    "usage: wayland-remote-viewer drive --addr ip:port [--fingerprint hex | --insecure] [--click x,y[,button]]* [--key scancode]* [--wait ms]* [--frames N] [--out dir]"
                );
                std::process::exit(0);
            }
            other => return Err(format!("unknown drive arg: {other}")),
        }
        i += 1;
    }

    let addr = addr.ok_or_else(|| "--addr is required".to_string())?;
    if !insecure && fingerprint.is_none() {
        return Err("--fingerprint <hex> or --insecure is required".to_string());
    }

    Ok(DriveConfig {
        addr,
        fingerprint,
        insecure,
        actions,
        max_frames,
        out_dir,
    })
}

fn fail(msg: &str) -> ! {
    eprintln!("viewer: {msg}");
    eprintln!(
        "usage: wayland-remote-viewer --addr ip:port [--fingerprint hex | --insecure] [--headless secs]"
    );
    std::process::exit(2);
}

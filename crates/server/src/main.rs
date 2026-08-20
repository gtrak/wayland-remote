//! Entry point for the wayland-remote server.
//!
//! Headless Wayland compositor (plan 001 issue 03) with the optional QUIC
//! frame server (issue 05): parses `--width`/`--height`/`--socket`/
//! `--snapshot`/`--listen`/`--raw`/`--fingerprint`, runs the compositor, and
//! exits on SIGINT/SIGTERM.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use wayland_remote_protocol::Compression;
use wayland_remote_server::net::cert::ServerCert;
use wayland_remote_server::state::Config;

const USAGE: &str = "usage: wayland-remote-server \
    [--width N] [--height N] [--socket NAME] [--snapshot PATH] \
    [--listen ADDR] [--raw] [--fingerprint]";

/// Parsed command line: the compositor config plus the `--fingerprint` flag.
struct Cli {
    config: Config,
    fingerprint: bool,
}

fn parse_args() -> Cli {
    // Networking is on by default; `--listen` overrides the address.
    let mut config = Config {
        listen: Some("0.0.0.0:9000".parse().expect("valid static default")),
        ..Config::default()
    };
    let mut fingerprint = false;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = match arg.as_str() {
            "--width" | "--height" | "--socket" | "--snapshot" | "--listen" => {
                args.next().unwrap_or_else(|| {
                    eprintln!("missing value for {arg}");
                    eprintln!("{USAGE}");
                    std::process::exit(2);
                })
            }
            "--help" | "-h" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            "--raw" => {
                config.compression = Compression::None;
                continue;
            }
            "--fingerprint" => {
                fingerprint = true;
                continue;
            }
            other => {
                eprintln!("unknown argument: {other}");
                eprintln!("{USAGE}");
                std::process::exit(2);
            }
        };

        match arg.as_str() {
            "--width" => {
                config.width = value.parse().unwrap_or_else(|_| {
                    eprintln!("invalid --width: {value}");
                    std::process::exit(2);
                });
            }
            "--height" => {
                config.height = value.parse().unwrap_or_else(|_| {
                    eprintln!("invalid --height: {value}");
                    std::process::exit(2);
                });
            }
            "--socket" => {
                config.socket_name = Some(value);
            }
            "--snapshot" => {
                config.snapshot = Some(PathBuf::from(value));
            }
            "--listen" => {
                config.listen = Some(value.parse::<SocketAddr>().unwrap_or_else(|_| {
                    eprintln!("invalid --listen: {value}");
                    std::process::exit(2);
                }));
            }
            _ => unreachable!("matched above"),
        }
    }
    Cli {
        config,
        fingerprint,
    }
}

fn main() {
    let cli = parse_args();
    if cli.fingerprint {
        match ServerCert::load_or_generate() {
            Ok(cert) => println!("{}", cert.fingerprint),
            Err(err) => {
                eprintln!("wayland-remote-server error: {err}");
                std::process::exit(1);
            }
        }
        return;
    }
    let shutdown = Arc::new(AtomicBool::new(false));
    if let Err(err) = wayland_remote_server::run(cli.config, shutdown, None, None) {
        eprintln!("wayland-remote-server error: {err}");
        std::process::exit(1);
    }
}

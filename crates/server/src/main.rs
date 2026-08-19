//! Entry point for the wayland-remote server.
//!
//! Headless Wayland compositor (plan 001 issue 03): parses
//! `--width`/`--height`/`--socket`, runs the compositor, and exits on
//! SIGINT/SIGTERM.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use wayland_remote_server::state::Config;

fn parse_args() -> Config {
    let mut config = Config::default();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        let value = match arg.as_str() {
            "--width" | "--height" | "--socket" => args.next().unwrap_or_else(|| {
                eprintln!("missing value for {arg}");
                eprintln!("usage: wayland-remote-server [--width N] [--height N] [--socket NAME]");
                std::process::exit(2);
            }),
            "--help" | "-h" => {
                println!("usage: wayland-remote-server [--width N] [--height N] [--socket NAME]");
                std::process::exit(0);
            }
            other => {
                eprintln!("unknown argument: {other}");
                eprintln!("usage: wayland-remote-server [--width N] [--height N] [--socket NAME]");
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
            _ => unreachable!("matched above"),
        }
    }
    config
}

fn main() {
    let config = parse_args();
    let shutdown = Arc::new(AtomicBool::new(false));
    if let Err(err) = wayland_remote_server::run(config, shutdown, None) {
        eprintln!("wayland-remote-server error: {err}");
        std::process::exit(1);
    }
}

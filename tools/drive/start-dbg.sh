#!/bin/bash
source $HOME/.cargo/env
cd $HOME/dev/wayland-remote
RUST_LOG=wayland_remote_server=debug ./target/release/wayland-remote-server --listen 0.0.0.0:9000 > /tmp/wr-dbg.log 2>&1 &
echo $!

#!/bin/bash
# Capture weston-simple-egl's Wayland protocol trace against the running server.
pkill -9 -f "[w]eston-simple-egl" 2>/dev/null
sleep 1
export XDG_RUNTIME_DIR=/run/user/1000
export WAYLAND_DISPLAY=wayland-1
timeout 5 env WAYLAND_DEBUG=1 weston-simple-egl > /tmp/wr-egldebug.log 2>&1
echo "captured $(wc -l < /tmp/wr-egldebug.log) lines"

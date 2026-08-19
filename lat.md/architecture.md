# Architecture

System architecture of wayland-remote: a headless Wayland compositor on Linux that streams rendered frames to a native Windows viewer, per [[decisions#Architecture Overview]].

## Crate Map

Three crates in one workspace. `protocol` is the pure wire-format library shared by both sides; `server` is the Smithay compositor plus QUIC endpoint; `viewer` is the Windows client.

## Runtime Split

The compositor runs on a single-threaded calloop event loop; all network I/O runs on a separate tokio runtime.

They communicate through channels: frames out via tokio mpsc with `blocking_send`, input events in via `calloop::channel`. The compositor thread never awaits; the network tasks never touch compositor state directly. See [[decisions#Decision Log#Runtime Split]].

## Rendering Pipeline

Client surfaces render offscreen with Smithay's pixman software renderer into a BGRA buffer that doubles as the wire payload.

wl_shm buffers import as pixman textures, the surface tree renders into a `PixmanTarget`, and readback yields a BGRA buffer with a real (padded) stride. That buffer — unchanged — is what goes on the wire, so GDI can blit it with zero conversion ([[decisions#Decision Log#BGRA Wire Format]]).

## QUIC Session Model

Each connection is one quinn session: a control stream plus one unidirectional stream per frame, with receiver-side skip-stale.

Control traffic (handshake, input, window events, ping/pong) shares one bidirectional stream; each compressed frame gets its own stream so a lost frame cannot head-of-line-block later frames. Receivers issue STOP_SENDING on stale frame streams — UDP-like drop-oldest semantics without custom loss recovery ([[decisions#Decision Log#Transport]]).

# Issue 05 — QUIC streaming server + wr-dump

## Objective

(PRD Step 3) Stream rendered frames over QUIC with quinn: self-signed TLS, per-frame unidirectional streams with skip-stale, bidirectional control stream, lz4-compressed payloads. Includes the `wr-dump` verification tool. This completes M1.

## Files

| File | Change |
|---|---|
| `crates/server/src/net/mod.rs` | `NetServer`: quinn `Endpoint` setup, accept loop, connection state machine |
| `crates/server/src/net/cert.rs` | Self-signed cert via rcgen (aws-lc-rs); persist keypair under `$XDG_DATA_HOME/wayland-remote/` (or `~/.local/share`); expose SPKI fingerprint (sha256, hex) for TOFU |
| `crates/server/src/net/session.rs` | Per-connection task: control stream (Hello/Welcome, Ping/Pong), frame sender with skip-stale bookkeeping |
| `crates/server/src/bridge.rs` | calloop ↔ tokio bridge: `calloop::channel` (events in), `tokio::sync::mpsc` + `blocking_send` (frames out); owns the tokio runtime handle |
| `crates/server/src/main.rs` | Flags: `--listen <ip:port>` (default `0.0.0.0:9000`), `--raw` (Compression::None), `--fingerprint` (print + exit); spawn tokio runtime alongside calloop loop; SIGINT tears both down |
| `crates/server/src/bin/wr-dump.rs` | QUIC client: TOFU connect, reads frame streams, decompresses, writes raw BGRA to stdout |
| `crates/server/tests/streaming.rs` | In-process end-to-end tests |

## Implementation notes

- **Runtime split** (the core invariant, see [[decisions#Runtime Split]]): the compositor thread never awaits. It pushes finished `FrameBuffer`s through an mpsc (`blocking_send` from calloop is fine — mpsc is unbounded; use a bounded wrapper with drop-oldest if backpressure is needed). The tokio side owns all quinn handles. Input events flow the reverse direction via `calloop::channel::sync_channel` registered in the loop.
- **quinn setup (0.11)**: `quinn::Endpoint::server(ServerConfig, addr)`; `ServerConfig::with_crypto(Arc<quinn::ServerConfig>)` — build rustls `ServerConfig` from rcgen cert + key with `aws-lc-rs` provider installed globally via `rustls::crypto::aws_lc_rs::default_provider().install_default()`. ALPN: `"wayland-remote/1"`.
- **Per-frame streams**: for each frame, `connection.open uni()`, write 32-byte `FrameHeader` + payload, finish. Track outstanding stream ids in a `BTreeMap<StreamId, frame_id>`; when a newer frame starts, `connection.stop_send` on all older ids (code 0). This is the skip-stale semantic — unit test it with a mock conn or a real loopback connection with artificial delay.
- **wr-dump**: client endpoint with `dangerous()`-style verifier that pins the expected fingerprint (required arg: `--fingerprint <hex>` or `WR_FINGERPRINT` env; or `--insecure` for local dev). Prints header info to stderr (frame count, size, decode time) so latency is observable. Writes **raw BGRA to stdout** for ffplay.
- **Frame pacing / drop policy**: if the mpsc from calloop holds >1 frame, the sender task drains and sends only the newest (coalescing) — bounds memory + bandwidth when the network is slower than the renderer.
- Version handshake: Hello/Welcome exchange; version mismatch → close with error code 1.
- Reuse connections: viewer reconnects cleanly (connection_closed → remove session, keep serving).

## Steps

1. `cert.rs` + fingerprint printing (test: same keypair loaded twice → same fingerprint).
2. `bridge.rs` with both channels; unit test round-trip message flow calloop→tokio→calloop.
3. `NetServer` + session: accept, handshake, ping/pong on control stream.
4. Frame sender with skip-stale + lz4 (respect `--raw`).
5. `wr-dump` bin.
6. Integration tests + `@lat:` refs; update `lat.md/` (net architecture, TOFU decision details, test specs).

## Verification

- Test `handshake_and_ping`: wr-dump-style client connects, Hello/Welcome versions match, Ping→Pong echoes timestamp.
- Test `version_mismatch_rejected`: client sends wrong version → stream error.
- Test `frame_roundtrip`: client commits pattern → received frame's header (w/h/stride/format) matches, decompressed pixels equal the issue-04 readback bytes exactly.
- Test `lz4_meets_budget`: compress a 1280x720 synthetic frame (UI-like: flat regions + text-like noise) in < 8ms median of 20 runs; if the safe-mode budget fails, this test documents the measured number and gates the fallback decision (lz4_flex unsafe feature) — do not silently weaken.
- Test `skip_stale_drops_old_streams`: slow-reading client + 3 rapid frames → client observes only the newest frame completes; older streams get STOP_SENDING (assert via quinn stream errors or absence of stale payloads).
- Test `frame_coalescing`: burst of 5 frames enqueued while sender busy → exactly 1 (newest) goes on the wire.
- Manual (PRD Step 3 form): `wayland-remote-server &` + test client, then `wr-dump --insecure --size 1280x720 | ffplay -f rawvideo -pixel_format bgra -video_size 1280x720 -` shows the live pattern.
- `cargo zigbuild` still green (aws-lc-rs cross via zig); CI green; `lat check` green.

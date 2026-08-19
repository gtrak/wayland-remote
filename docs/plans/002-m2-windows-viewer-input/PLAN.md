# Plan 002 — M2: Windows Viewer + Input Round-trip

## Why

M1 ([[001-m1-streaming-linux|Plan 001]]) streams frames from Linux. M2 puts pixels on a Windows desktop and closes the loop: keyboard and mouse events from Windows reach Wayland clients. This is the PRD's "you should see the Linux window" + "type in remote terminal" milestone.

## What

- **Viewer**: a Win32 application (windows-sys, no winit) that connects over QUIC (TOFU fingerprint), decodes frames, displays them in a window via GDI `StretchDIBits`, and sends input events upstream on the control stream.
- **Input injection** (server): incoming `Input` messages are injected into the smithay seat via `KeyboardHandle`/`PointerHandle` with the xkb keymap translating Windows scancodes → keysyms → clients.

## Success criteria

- Cross-compiled exe (and/or native MSVC build) on a real Windows box displays the streamed desktop live.
- Typing on the Windows keyboard produces correct characters in a remote terminal running under the compositor; mouse moves/clicks/scroll hit the right surface location.
- Measured round-trip (Ping/Pong) and frame latency printed by viewer (stretch goal: on-screen overlay).
- All viewer logic that is not Win32-specific (protocol handling, session state, input event construction) is unit-tested on Linux; CI runs those tests.

## Task order

```
01-viewer-app ──> 02-input-injection
```

Issue 01 needs M1's server + wr-dump client as a starting template (reuse `cert`/TOFU verifier logic from `crates/server/src/net/cert.rs` — factor shared bits into `crates/protocol` or a small `crates/common` if duplication exceeds ~50 lines).

## Scope

In: viewer window + GDI blit + resize + TOFU + reconnect, scancode translation, server-side input injection, latency instrumentation, Linux-runnable viewer unit tests.

Out: multi-window HWND mapping ([[003-m3|Plan 003]] — M2 viewer is a single full-desktop window), cursor rendering (server-side cursor sprite is M3 polish), lossy/WAN tuning, clipboard.

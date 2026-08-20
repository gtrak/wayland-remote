# 07 — Additional Globals (wl_shell + zwp_text_input_v3)

## Objective

Broaden legacy and terminal app compatibility: add `wl_shell` (so older clients
like `weston-simple-shm` map) and `zwp_text_input_v3` (so `weston-terminal`
stops warning and IME-aware clients get a text-input manager). Both are small
delegate additions with minimal handlers.

## Files

| File | Change |
|------|--------|
| `crates/server/src/state.rs` | Add `ShellState` (legacy `wl_shell`) + `TextInputManagerState` (text-input-v3); fields on `State`; `delegate_shell!` / `delegate_text_input_v3!`. |
| `crates/server/Cargo.toml` | Confirm smithay features for shell + text_input are enabled. |
| `crates/server/src/window.rs` | Handle `wl_shell` surface mapping the same way as xdg toplevels (register → configure-equivalent → Created on first commit). |

## Steps

1. `wl_shell`: add `ShellState::new::<State>(&dh)`; implement `ShellHandler` with `ping` (noop) and `get_shell_surface` mapping a legacy surface into the `WindowManager` (treat a `wl_shell_surface` toplevel like an xdg toplevel: send an initial configure, map on first commit). Add `delegate_shell!(State)`.
2. `zwp_text_input_v3`: add `TextInputManagerState::new::<State>(&dh)`; implement `TextInputHandler` with default/noop methods (headless compositor has no IME engine — the global's presence stops the "No text input manager" warning and lets clients bind it; typing still works via the keyboard path fixed in 05). Add `delegate_text_input_v3!(State)`.
3. Keep handlers minimal; do not implement actual IME composition (out of scope — typing works via keyboard). Verify the trait method signatures against the smithay 0.7 source on gary-agents (`/home/gary/.cargo/registry/.../smithay-0.7.0/src/wayland/`).

## Verification

- `cargo test -p wayland-remote-server` green.
- Live on gary-agents: `weston-simple-shm` connects and renders (no `wl_shell` missing error); `weston-terminal` no longer warns "No text input manager global" and typed text appears.
- A unit test asserting the globals appear in the registry (extend the `common/mod.rs` registry bind list).
- `lat check` green.

---
name: windows-sys-win32
description: >-
  Exact windows-sys 0.61 Win32 message-loop + GDI StretchDIBits FFI patterns
  for the wayland-remote viewer. Read this before writing or editing
  crates/viewer/src/display/win.rs — do NOT re-derive the FFI from docs.
---

# windows-sys 0.61 Win32 Patterns for the Viewer

Exact types, signatures, and wiring patterns for the Windows viewer's
`display/win.rs` (the only FFI module in the project). Verified against
`windows-sys = "0.61.2"` as pinned in the workspace `Cargo.toml`. Use these
directly; do not re-derive from `docs.rs` or guess at signatures.

## Cargo features (already declared — do not add more)

The workspace `Cargo.toml` declares on `windows-sys`:

```toml
windows-sys = { version = "0.61.2", features = [
    "Win32_Graphics_Gdi",
    "Win32_UI_WindowsAndMessaging",
    "Win32_Foundation",
    "Win32_System_LibraryLoader",
    "Win32_UI_Input_KeyboardAndMouse",
] }
```

The viewer pulls it in only on Windows (`[target.'cfg(windows)'.dependencies]`
in `crates/viewer/Cargo.toml`). Every type/function below is gated behind one
of these features and is available with no further feature additions.

## `unsafe` and the workspace lint

The workspace sets `[workspace.lints.rust] unsafe_code = "deny"`. Win32 FFI
requires `unsafe`, so the **only** accepted override is an inner attribute at
the very top of `crates/viewer/src/display/win.rs`:

```rust
//! Win32 display layer (only compiled on Windows).
#![allow(unsafe_code)]

// ... rest of file uses `unsafe { ... }` freely ...
```

Put `#![allow(unsafe_code)]` (inner attribute, `!`) on line 1 of the module
body. Do NOT add `unsafe` anywhere else in the crate. Do NOT use
`#[allow(unsafe_code)]` on individual items — the module-level allow is the
single, reviewable exception.

## Edition 2024 `unsafe extern` blocks

The workspace is `edition = "2024"`. In edition 2024, **extern item declarations
inside an `extern` block must be `unsafe`**, and foreign-function-import
`extern` blocks are themselves `unsafe extern { ... }`. BUT: the WndProc is
NOT an imported foreign function — it is a Rust function with the `"system"`
ABI that you pass to Win32 as a callback. So WndProc is declared as a normal
Rust function with an `unsafe` body:

```rust
unsafe extern "system" fn controller_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT { ... }
```

Exact type aliases in windows-sys 0.61.2 (verified from source — use these,
do not guess):

| Type | Definition | Notes |
|---|---|---|
| `HWND` | `*mut core::ffi::c_void` | raw pointer; **no `HWND::null()` method** — use `0 as HWND` or `core::ptr::null_mut()` |
| `HINSTANCE` | `*mut core::ffi::c_void` | same; `GetModuleHandleW` returns `HMODULE` (also `*mut c_void`) — interchangeable, no cast |
| `HDC` | `*mut core::ffi::c_void` | |
| `WPARAM` | `usize` | |
| `LPARAM` | `isize` | |
| `LRESULT` | `isize` | |
| `BOOL` | `i32` | `GetMessageW`/`PostMessageW` return this; `0` = false |
| `PCWSTR` | `*const u16` | |

All the `HWND`-family are raw pointers (NOT newtypes), so they are `Copy`,
have no methods, and use `0 as HWND` for null. Constants like
`CW_USEDEFAULT` (`i32 = -2147483648`, i.e. `i32::MIN`/`0x80000000`), the
`WM_*`/`WS_*`/`CS_*`/`SWP_*` message codes, and `BI_RGB`/`DIB_RGB_COLORS`/
`SRCCOPY` are plain `u32`/`i32` type aliases (e.g. `WINDOW_STYLE = u32`,
`WNDCLASS_STYLES = u32`, `SET_WINDOW_POS_FLAGS = u32`, `WINDOW_LONG_PTR_INDEX = i32`),
so `WS_OVERLAPPEDWINDOW | WS_VISIBLE` works via `u32`'s `BitOr` directly.

`WNDCLASSW`, `MSG`, `BITMAPINFO`, `BITMAPINFOHEADER`, `RGBQUAD`, `RECT`, and
`PAINTSTRUCT` all implement `Default` (via `unsafe core::mem::zeroed()` under
the feature flags you already have). **Prefer `let mut wc = WNDCLASSW::default()`
then set fields by name** over a full struct literal — it avoids field-order
mistakes and lets you leave the irrelevant fields zeroed.

`WNDPROC = Option<unsafe extern "system" fn(HWND, u32, WPARAM, LPARAM) -> LRESULT>`,
so `lpfnWndProc: Some(controller_proc)` is the correct wrapping.

Functions (`RegisterClassW`, `CreateWindowExW`, `PostMessageW`,
`StretchDIBits`, …) are declared in `windows-sys` and called inside
`unsafe { ... }` blocks — they are NOT `extern` blocks you write; they are
imported items from the crate.

## Window class registration + WndProc

```rust
use windows_sys::Win32::Foundation::{HWND, LPARAM, WPARAM, LRESULT, HINSTANCE};
use windows_sys::Win32::UI::WindowsAndMessaging::{
    WNDCLASSW, RegisterClassW, CreateWindowExW, DefWindowProcW,
    CS_HREDRAW, CS_VREDRAW, WM_DESTROY, GWLP_USERDATA, /* etc */
};

// A class name as a null-terminated UTF-16 literal:
const CONTROLLER_CLASS: &[u16] = widename("WaylandRemoteController");
const CHILD_CLASS: &[u16] = widename("WaylandRemoteChild");

// Helper: turn a &str into a null-terminated UTF-16 slice. A `[u16]` with a
// trailing 0 is what `WNDCLASSW::lpszClassName` expects (`PCWSTR` = `*const u16`).
fn widename(s: &str) -> [u16] { /* encode_utf16 + push 0 */ }
// For a runtime title, build a Vec<u16> ending in 0 and pass `.as_ptr()`.

unsafe extern "system" fn controller_proc(
    hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM,
) -> LRESULT {
    // Default for everything you don't handle:
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
```

Register:

```rust
let mut wc = WNDCLASSW::default();       // implements Default; zeroed fields
wc.style = CS_HREDRAW | CS_VREDRAW;      // WNDCLASS_STYLES = u32; BitOr works
wc.lpfnWndProc = Some(controller_proc);  // WNDPROC = Option<unsafe extern "system" fn(...) -> LRESULT>
wc.hInstance = hinst;                    // from GetModuleHandleW(null) — returns HMODULE, same type
wc.lpszClassName = CONTROLLER_CLASS.as_ptr();  // PCWSTR = *const u16
wc.hCursor = unsafe { LoadCursorW(0 as HINSTANCE, IDC_ARROW) };  // IDC_ARROW is a PCWSTR
// leave hbrBackground = 0 (no bg brush; WM_PAINT paints everything), hIcon = 0, etc.
let atom = unsafe { RegisterClassW(&wc) };   // 0 = error
assert!(atom != 0, "RegisterClassW failed");
```

`CONTROLLER_CLASS` is a `&[u16]` with a trailing `0` (null-terminated UTF-16);
`as_ptr()` gives the `*const u16` that `lpszClassName` wants. Build it with a
helper that `encode_utf16`s the name and pushes a `0`.

## Per-window state via `GWLP_USERDATA`

`SetWindowLongPtrW` / `GetWindowLongPtrW` store a pointer-sized value on each
HWND. The standard pattern: allocate per-window state on the heap as a `Box`,
store its raw pointer (cast to `isize`) on the HWND, recover it in the WndProc,
and free it on `WM_DESTROY` (or when the controller's message loop exits).

```rust
use windows_sys::Win32::UI::WindowsAndMessaging::{SetWindowLongPtrW, GetWindowLongPtrW, GWLP_USERDATA};

struct UiState {
    shared: Arc<Shared>,
}

// After CreateWindowExW returns the controller HWND:
let state = Box::new(UiState { shared: Arc::clone(&shared) });
unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(state) as isize) };

// In the WndProc, recover:
let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut UiState;
// Before the pointer is set (the first few messages: WM_GETMINMAXINFO, WM_CREATE,
// WM_NCCREATE), state_ptr is 0 — guard with `if state_ptr.is_null()` and fall
// through to DefWindowProcW.

// Free at the end of run() (after the message loop exits), NOT in WM_DESTROY of
// the controller — the controller window outlives individual messages:
let _ = unsafe { Box::from_raw(state_ptr) };
```

`GWLP_USERDATA` is `-21` (`i32`); it is the documented per-window user slot.
`SetWindowLongPtrW` returns the previous value (0 first time).

## The message loop

```rust
use windows_sys::Win32::UI::WindowsAndMessaging::{MSG, GetMessageW, TranslateMessage, DispatchMessageW};

let mut msg = MSG::default();   // MSG: Default-able; all fields are integers/pointers
loop {
    // GetMessageW returns BOOL (= i32): 1 = message retrieved, 0 = WM_QUIT (break), -1 = error (break).
    let r = unsafe { GetMessageW(&mut msg, 0 as HWND, 0, 0) };
    if r <= 0 { break; }            // 0 (quit) and -1 (error) both exit the loop
    unsafe { TranslateMessage(&msg) };
    unsafe { DispatchMessageW(&msg) };
}
```

`HWND` is a raw `*mut c_void` (NOT a newtype), so there is no `HWND::null()`
method — use `0 as HWND` (or `core::ptr::null_mut()` cast). `MSG` implements
`Default`.

To exit: `unsafe { PostQuitMessage(0) }` from any WndProc — `GetMessageW` then
returns 0 on the next iteration and the loop breaks.

## Cross-thread signaling: `PostMessageW`

The net task (a background thread) must not touch GDI; it signals the UI
thread by posting messages. `PostMessageW` is thread-safe and non-blocking:

```rust
use windows_sys::Win32::UI::WindowsAndMessaging::PostMessageW;

const WM_USER: u32 = 0x0400;
const WM_USER_FRAME: u32     = WM_USER + 1;  // wparam = window_id (u64 cast to usize)
const WM_USER_WIN_EVENT: u32 = WM_USER + 2;  // wparam = window_id, lparam = Box::into_raw(ptr) as isize
const WM_USER_RTT: u32       = WM_USER + 3;  // wparam = rtt_ms (usize)
const WM_USER_NET_CLOSED: u32 = WM_USER + 4;

// Net thread:
unsafe { PostMessageW(controller_hwnd, WM_USER_FRAME, frame.window_id as usize, 0) };

// To pass an owned value through lParam, box it on the heap and pass the raw
// pointer; the UI thread takes ownership and frees it:
let boxed = Box::new(event);                 // event: WindowEventKind (Send)
let raw = Box::into_raw(boxed) as isize;
unsafe { PostMessageW(controller_hwnd, WM_USER_WIN_EVENT, wid as usize, raw) };

// UI thread handler:
let event = unsafe { Box::from_raw(lparam as *mut WindowEventKind) };
// use *event, then it drops when `event` goes out of scope
```

`WPARAM` and `LPARAM` are `isize`-typed aliases. `PostMessageW` returns a
`BOOL` (i32, 0 = failure — the target window may be gone; ignore failures on
the net thread). The posted message is delivered on the UI thread's
`GetMessageW`/`DispatchMessageW` and handled in the WndProc.

## Background thread + tokio runtime

```rust
use std::thread;

let net_handle = thread::spawn(move || {
    // Build a single-threaded tokio runtime ON this background thread.
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()      // needs time (for ping interval) + net
        .build()
        .expect("tokio runtime");
    rt.block_on(net_main(addr, fingerprint, insecure, controller_hwnd, shared));
});
```

The UI thread owns the HWNDs and the message loop; the net thread owns the
QUIC session (`ViewerSession`). They communicate via `Arc<Shared>` (shared
state: per-window `FrameStore` map, a `tokio::mpsc` input sender, a
`tokio::mpsc` control-command sender) and `PostMessageW` (net→UI only).
`Shared` is `Send + Sync` because all its fields are `Mutex`/`mpsc::Sender`.

`Arc<Shared>` is cloned into `UiState` (for the controller's GWLP_USERDATA)
and into the net task's closure (the `move`). Both keep it alive.

## Async orchestration: the `select!` borrow conflict (IMPORTANT)

The net task must concurrently: receive frames, send input/control upstream,
drain control messages, and ping. A single `tokio::select!` over `ViewerSession`
**will not compile** because the branches borrow the session at different
mutability levels simultaneously:

- `session.next_frame()` takes `&self` (it only uses `self.conn`).
- `session.send_input()` / `send_control()` / `try_read_control()` take `&mut self` (they use `self.ctrl_send`/`self.ctrl_recv`).

`select!` builds all branch futures before polling, so `&self` and `&mut self`
in the same macro = a borrow-checker error.

**The fix**: `quinn::Connection` is `Clone` (cheap, `Arc`-based internally) and
`next_frame` only needs `self.conn`. Clone the connection and spawn the
frame-receive loop as a **separate tokio task**; the main net task keeps
`&mut ViewerSession` for the control loop. Add a small accessor:

```rust
// crates/viewer/src/session.rs
impl ViewerSession {
    /// A cheap clone of the QUIC connection, for spawning the frame-receive
    /// loop on its own task (it only needs `&Connection`, not `&mut self`).
    pub fn connection(&self) -> quinn::Connection { self.conn.clone() }
}
```

Net task structure:

```rust
async fn net_main(addr, fingerprint, insecure, controller_isize, shared: Arc<Shared>) {
    let controller = controller_isize as HWND;
    let mut session = match ViewerSession::connect(addr, fingerprint, insecure).await {
        Ok(s) => s,
        Err(e) => { eprintln!("viewer: connect failed: {e:?}"); unsafe { PostMessageW(controller, WM_USER_NET_CLOSED, 0, 0) }; return; }
    };

    // Frame-receive task: owns a cloned Connection, swaps frames into `shared.frames`,
    // posts WM_USER_FRAME. Runs until the stream errors (connection closed).
    let conn = session.connection();
    let frame_shared = Arc::clone(&shared);
    tokio::spawn(async move {
        loop {
            // Reimplement next_frame's body inline against `conn` (a fresh ViewerSession
            // method `read_frame_from(&conn)` is cleaner — add one that mirrors
            // next_frame but takes `&quinn::Connection`):
            match read_frame(&conn).await {   // see below
                Ok(frame) => {
                    let wid = frame.window_id;
                    frame_shared.frames.lock().unwrap()
                        .entry(wid).or_insert_with(|| Arc::new(FrameStore::new()))
                        .swap(frame);
                    unsafe { PostMessageW(controller, WM_USER_FRAME, wid as usize, 0) };
                }
                Err(_) => { unsafe { PostMessageW(controller, WM_USER_NET_CLOSED, 0, 0) }; break; }
            }
        }
    });

    // Control loop: owns `&mut session`. Inputs, control commands, pings, control reads.
    let (input_rx, control_rx) = /* receivers extracted before spawning, or stored in shared */;
    let mut ping = tokio::time::interval(Duration::from_millis(500));
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut pending_ping: Option<u64> = None;
    loop {
        tokio::select! {
            // All branches borrow `&mut session` only — no conflict.
            Some((wid, ev)) = input_rx.recv() => { let _ = session.send_input(wid, ev).await; }
            Some(cmd) = control_rx.recv() => {
                match cmd {
                    ControlCommand::SetFocus(w)        => { let _ = session.send_control(&Message::SetFocus{window_id:w}).await; }
                    ControlCommand::CloseWindow(w)     => { let _ = session.send_control(&Message::CloseWindow{window_id:w}).await; }
                    ControlCommand::ConfigureWindow(w,cw,ch) => { let _ = session.send_control(&Message::ConfigureWindow{window_id:w,width:cw,height:ch}).await; }
                    ControlCommand::Shutdown => { session.close(); unsafe { PostMessageW(controller, WM_USER_NET_CLOSED, 0, 0) }; break; }
                }
            }
            _ = ping.tick() => {
                let ts = now_ns();
                pending_ping = Some(ts);
                let _ = session.send_control(&Message::Ping{timestamp_ns: ts}).await;
            }
            // Poll control messages with a short timeout so the select! stays responsive:
            maybe = async {
                match tokio::time::timeout(Duration::from_millis(50), session.read_control()).await {
                    Ok(Ok(msg)) => Some(msg),
                    _ => None,
                }
            } => {
                if let Some(Message::WindowEvent{window_id,event}) = maybe {
                    let raw = Box::into_raw(Box::new(event)) as isize;
                    unsafe { PostMessageW(controller, WM_USER_WIN_EVENT, window_id as usize, raw) };
                } else if let Some(Message::Pong{timestamp_ns}) = maybe {
                    if Some(timestamp_ns) == pending_ping {
                        let rtt = now_ns().saturating_sub(timestamp_ns) / 1_000_000;
                        unsafe { PostMessageW(controller, WM_USER_RTT, rtt as usize, 0) };
                    }
                } else if let Some(Message::Ping{timestamp_ns}) = maybe {
                    let _ = session.send_control(&Message::Pong{timestamp_ns}).await;
                }
            }
        }
    }
}
```

`read_frame(&conn)` is `next_frame`'s body lifted to take `&quinn::Connection`
(add it as a `pub async fn read_frame(conn: &quinn::Connection) -> anyhow::Result<FrameBuffer>`
on `ViewerSession`, and have `next_frame` call it for one source of truth).
`now_ns()` is `SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(0)`.

Because the frame task and the control loop now use **separate** values (a
cloned `Connection` vs `&mut ViewerSession`), there is no borrow conflict. The
frame task exits when `read_frame` errors (connection closed); the control loop
exits on `Shutdown` or a send error. Either posts `WM_USER_NET_CLOSED` to quit
the UI loop. The UI thread's `WM_CLOSE` sends `Shutdown` to end the net task
cleanly.

**Receivers**: create both `tokio::mpsc::unbounded_channel`s in `run()` BEFORE
spawning the net thread; store the senders in `Arc<Shared>` and move the
receivers into the net task closure (they are `Send`). The control loop above
closes over `input_rx` and `control_rx` by move.

## Per-window frame store + blit (`WM_PAINT`)

`FrameStore` (already implemented in `framebuf.rs`) is `Send + Sync`. Keep a
`Arc<Mutex<HashMap<u64, Arc<FrameStore>>>>` in `Shared`. The net task swaps
frames in; the UI child WndProc reads them on `WM_PAINT`.

```rust
use windows_sys::Win32::Graphics::Gdi::{
    StretchDIBits, BeginPaint, EndPaint, PAINTSTRUCT,
    BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{WM_PAINT, GetClientRect};

// In the child WndProc, WM_PAINT:
let mut ps = PAINTSTRUCT::default();
let hdc = unsafe { BeginPaint(hwnd, &mut ps) };   // HDC = *mut c_void
// Get the latest frame for this window_id:
if let Some(frame) = /* shared.frames.lock().get(&window_id).and_then(|s| s.latest()) */ {
    let mut r = RECT::default();
    unsafe { GetClientRect(hwnd, &mut r) };
    let cw = r.right - r.left;
    let ch = r.bottom - r.top;
    let mut info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: frame.width as i32,
            biHeight: -(frame.height as i32),   // NEGATIVE = top-down rows.
                                                // Pixman readback is BGRA
                                                // little-endian; GDI 32bpp
                                                // BI_RGB reads it as BGRA. Zero conversion.
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB,
            biSizeImage: 0, biXPelsPerMeter: 0, biYPelsPerMeter: 0,
            biClrUsed: 0, biClrImportant: 0,
        },
        bmiColors: [0; 1],   // RGBQUAD[1]; unused for BI_RGB. BITMAPINFO's trailing union.
    };
    unsafe {
        StretchDIBits(
            hdc,
            0, 0, cw, ch,              // destination (client rect) — stretch to fit
            0, 0, frame.width as i32, frame.height as i32,  // source
            frame.data.as_ptr() as *const c_void,
            &info,
            DIB_RGB_COLORS,
            SRCCOPY,
        );
    }
}
unsafe { EndPaint(hwnd, &ps) };
return 0;
```

Key facts (do not second-guess):
- `biHeight = -(height as i32)`: negative height = top-down image (rows
  start at the top). Pixman's `Argb8888` readback on little-endian is laid out
  in memory as `[B, G, R, A]` per pixel, top row first. GDI 32bpp `BI_RGB`
  with negative height reads the same `[B, G, R, A]` layout. No swap, no flip.
- `BITMAPINFO` ends with `bmiColors: [RGBQUAD; 1]` — a 1-element array is
  sufficient for `BI_RGB` (the palette is unused). Initialize with
  `RGBQUAD { rgbBlue:0, rgbGreen:0, rgbRed:0, rgbReserved:0 }` or via
  `BITMAPINFO::default()` then set the header fields.
- `StretchDIBits` returns the number of scan lines copied (0 on failure).
  Stretch-to-fit is the M2/M3 decision (documented in `lat.md/decisions.md`).
- `WM_ERASEBKGND` (0x0014): return 1 to suppress the default erase (avoids
  flicker; `WM_PAINT` paints the entire client area each time).

### Stride packing (correctness guard)

`StretchDIBits` with `BI_RGB` 32bpp computes the source row stride as
`biWidth * 4` (32bpp rows are always 4-byte-aligned). But `FrameHeader::stride`
in the protocol **may exceed `width * 4`** (pixman can pad rows). In this
project the server currently sets `stride = width * 4` (contiguous), but the
wire format permits padding. If `frame.stride != frame.width * 4`, passing
`frame.data.as_ptr()` directly to `StretchDIBits` would misread every row
after the first. Guard it by packing to a contiguous buffer when needed:

```rust
let tight = frame.stride as usize == frame.width as usize * 4;
let bits: *const c_void;
let _packed: Vec<u8>;   // keeps the packed buffer alive across the call
if tight {
    bits = frame.data.as_ptr() as *const c_void;
} else {
    let row = frame.width as usize * 4;
    let mut p = vec![0u8; row * frame.height as usize];
    for y in 0..frame.height as usize {
        let src = y * frame.stride as usize;
        p[y * row .. y * row + row].copy_from_slice(&frame.data[src .. src + row]);
    }
    _packed = p;
    bits = _packed.as_ptr() as *const c_void;
}
// pass `bits` to StretchDIBits; `_packed` lives until end of scope.
```

This is cheap (a `memcpy` only when the server pads, which it currently does
not) and correct. Prefer `BITMAPINFO::default()` then set header fields over a
full struct literal, so the `bmiColors` and the unused header fields are zeroed
without you listing them.

## Custom cursor (HCURSOR from a BGRA sprite)

The server sends cursor shapes as a 32-bit BGRA sprite (raw `width*height*4`
bytes, top-down, `[B, G, R, A]` per pixel — the same pixman readback layout the
`StretchDIBits` blit above consumes). Turn it into a native `HCURSOR` and drive
show/position/hide/destroy from the UI thread.

**Feature note (corrects the common assumption):** ALL five cursor calls live
in `Win32_UI_WindowsAndMessaging` (they are `user32.dll` exports), NOT in
`Win32_Graphics_Gdi` or `Win32_UI_Input_KeyboardAndMouse`. That feature is
already declared, so there is **no feature gap** — do not add any.

Verified against `windows-sys = "0.61.2"`
(`registry/.../windows-sys-0.61.2/src/Windows/Win32/UI/WindowsAndMessaging/mod.rs`,
lines 55/85/383/384/432 — none is cfg-gated beyond the module's own
`Win32_UI_WindowsAndMessaging` gate, confirmed at `Win32/UI/mod.rs:21`):

| Function | windows-sys 0.61.2 signature |
|---|---|
| `CreateCursor` | `(hinst: HINSTANCE, xhotspot: i32, yhotspot: i32, nwidth: i32, nheight: i32, pvandplane: *const c_void, pvxorplane: *const c_void) -> HCURSOR` |
| `SetCursor` | `(hcursor: HCURSOR) -> HCURSOR` (returns the **previous** thread cursor) |
| `SetCursorPos` | `(x: i32, y: i32) -> BOOL` (screen-absolute) |
| `ShowCursor` | `(bshow: BOOL) -> i32` (returns the new reference count) |
| `DestroyCursor` | `(hcursor: HCURSOR) -> BOOL` |

Types: `HCURSOR = *mut c_void` (defined in `Win32::UI::WindowsAndMessaging`,
so import it from there — it is **not** in `Foundation`); `HINSTANCE = *mut
c_void` (`Foundation`); `BOOL = i32` (`windows_sys::core`). All raw pointers /
integers, so `0 as HCURSOR` is the null handle and values are `Copy`.

```rust
use windows_sys::Win32::Foundation::HINSTANCE;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateCursor, SetCursor, SetCursorPos, ShowCursor, DestroyCursor, HCURSOR,
};

/// Build a native 32-bit alpha cursor from a top-down BGRA sprite.
/// `data` must be exactly `width*height*4` bytes, top-down, `[B,G,R,A]`/pixel
/// (little-endian `0x00AABBGGRR`) — the pixman/server readback layout, unchanged.
/// Returns a caller-owned HCURSOR (0 on failure); DestroyCursor it before
/// replacing or at shutdown. Run on the UI thread.
fn make_cursor(data: &[u8], width: u32, height: u32, hot_x: i32, hot_y: i32) -> HCURSOR {
    assert_eq!(data.len(), (width * height * 4) as usize, "sprite is width*height*4 BGRA");
    unsafe {
        CreateCursor(
            0 as HINSTANCE,                          // hInstance (unused for cursor data)
            hot_x, hot_y,                            // i32, signed — negative/edge hotspots OK
            width as i32, height as i32,
            core::ptr::null(),                       // pvAndPlane = NULL (32-bit alpha path)
            data.as_ptr() as *const core::ffi::c_void, // pvXorPlane = the BGRA sprite
        )
    }
}

// Drive it (UI thread only):
let hcur = make_cursor(&sprite.data, sprite.width, sprite.height, sprite.hot_x, sprite.hot_y);
if !hcur.is_null() {
    unsafe {
        SetCursor(hcur);     // sets the *thread* cursor; returns the previous HCURSOR
        SetCursorPos(x, y);  // move the pointer to a screen-absolute (x, y)
    }
}
// ... later, before overwriting `hcur` or at shutdown:
if !hcur.is_null() { unsafe { DestroyCursor(hcur) }; }
```

`&[u8]` is what you pass in; a `Vec<u8>` coerces to it. `CreateCursor` **copies**
the pixel data into the new cursor, so `data` only needs to be live for the call
(the returned `HCURSOR` is independent of the source buffer afterwards).

**The AND-mask decision (the fiddly part) — pass `NULL`.** For a 32-bit cursor,
Windows derives per-pixel transparency from the **alpha byte of the XOR mask**
(the high byte of each 32-bit BGRA pixel): `0` = fully transparent, `255` =
fully opaque. That path is selected **precisely** by passing `pvAndPlane =
NULL`. A 1-bit `pvAndPlane` would (a) be laid out **bottom-up**, 1 bit/pixel,
rows padded to 32 bits, and (b) force any pixel whose AND bit is set fully
transparent *regardless of its alpha byte* — the legacy 1-bit-cursor model,
extra work, and wrong for a smooth-alpha sprite. So: XOR = BGRA sprite, AND =
`NULL`.

**Byte order / flip (no conversion):** `CreateCursor` wants the XOR mask
top-down, 32 bits/pixel — exactly the sprite's native layout (identical to the
`StretchDIBits` blit above). No byte swap, no vertical flip. (Unlike a DIB, where
a positive `biHeight` means bottom-up, a cursor's `nheight` is always positive
and its rows are always top-down.)

**Pre-multiplied-alpha caveat:** the RGB channels must be **straight**
(un-premultiplied) — which is what pixman's `Argb8888` readback gives. If a
semi-transparent anti-aliased edge shows a color fringe, premultiply
`RGB *= A/255` before creating. Not needed for the usual opaque-center cursor.

**Hotspot:** `xhotspot`/`yhotspot` are **`i32` (signed)** — pass `hot_x`/`hot_y`
straight through; a negative or out-of-bounds hotspot is legal (the cursor is
positioned so that pixel sits on the pointer).

**Reference counting (`ShowCursor`):** initial count is `0` (visible).
`ShowCursor(0)` (FALSE) decrements → hidden at `< 0`; `ShowCursor(1)` (TRUE)
increments → visible again at `>= 0`. Every hide must be paired with a later
show, so track a hidden flag rather than calling blindly; the return value is the
new count.

**Thread:** call all five from the UI thread (the "only the UI thread touches
the cursor/GDI" rule). `SetCursor` sets the *thread* cursor, so run it on the
thread that pumps the window's messages; `SetCursorPos`/`ShowCursor` are
process-wide but do them on the UI thread for consistency.

## Input translation (WM_KEY*, WM_MOUSE*, WM_MOUSEWHEEL)

All pure-logic translation already exists in `crates/viewer/src/input.rs`
(`extract_scancode`, `key_event`, `pointer_move`, `pointer_button`, `scroll`).
The child WndProc just calls these and sends `InputEvent`s upstream via the
shared `tokio::mpsc::UnboundedSender<(u64, InputEvent)>`. The net task
receives and calls `session.send_input(window_id, event)`.

Message constants (do not re-import these as features; they are `pub const`
values in `Win32_UI_WindowsAndMessaging`):
- `WM_KEYDOWN` = 0x0100, `WM_KEYUP` = 0x0101
- `WM_MOUSEMOVE` = 0x0200, `WM_LBUTTONDOWN` = 0x0201, `WM_LBUTTONUP` = 0x0202,
  `WM_RBUTTONDOWN` = 0x0204, `WM_RBUTTONUP` = 0x0205,
  `WM_MBUTTONDOWN` = 0x0207, `WM_MBUTTONUP` = 0x0208,
  `WM_MOUSEWHEEL` = 0x020A
- `WM_ACTIVATE` = 0x0006, `WM_SIZE` = 0x0005, `WM_CLOSE` = 0x0010,
  `WM_DESTROY` = 0x0002, `WM_PAINT` = 0x000F, `WM_ERASEBKGND` = 0x0014

For `WM_SIZE`: `LOWORD(lparam)` = client width, `HIWORD(lparam)` = client
height (`(lparam & 0xFFFF) as u32`, `((lparam >> 16) & 0xFFFF) as u32`).

For `WM_MOUSEWHEEL`, `wparam` HIWORD is a signed `i16` delta; the pure
`input::scroll` already handles the cast.

`InputEvent::PointerButton { button, state }` carries NO coordinates (the
pointer position is established by the preceding `PointerMove`). Only
`PointerMove` needs coordinate scaling (surface_size / client_size) — and
only when a frame is known; otherwise send identity coords.

## Resize round-trip (avoid the echo loop)

When the user drags a child window border:
1. `WM_SIZE` arrives → send `Message::ConfigureWindow { window_id, w, h }`
   upstream and set a `pending_configure = true` flag on that child's state
   (stored in `ChildState` via the child's `GWLP_USERDATA`).
2. The server configures the toplevel, the client acks + commits, the server
   renders the new size and emits `WindowEventKind::Resized { width, height }`.
3. The controller's `WM_USER_WIN_EVENT` handler for `Resized` calls
   `SetWindowPos(child, ..., width, height, SWP_NOMOVE | SWP_NOZORDER)` and
   CLEARS `pending_configure`.
4. While `pending_configure` is set, ignore `WM_SIZE` (it's the echo of the
   `SetWindowPos` we did, not a user drag) — this breaks the feedback loop.

`SWP_NOMOVE` = 0x0002, `SWP_NOZORDER` = 0x0004. `SetWindowPos` takes
`(hwnd, hwnd_insert_after, x, y, cx, cy, flags)`; pass `HWND::null()` for the
insert-after with `SWP_NOZORDER`.

## Close + focus round-trips

- Child `WM_CLOSE` (X button): send `Message::CloseWindow { window_id }`
  upstream. Do NOT `DestroyWindow` locally — wait for the server's
  `WindowEventKind::Destroyed`, which the controller's `WM_USER_WIN_EVENT`
  handler turns into `DestroyWindow`. (Matches "the app decides to close".)
- Child `WM_ACTIVATE` (activated, low word of wparam = 1): send
  `Message::SetFocus { window_id }` upstream and record the focused window_id
  in `Shared` (a `Mutex<Option<u64>>`) for the RTT title update.

## `HINSTANCE`

```rust
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
let hinst = unsafe { GetModuleHandleW(0 as *const u16) };  // HINSTANCE; 0 = current process
```
Pass `hinst` to `WNDCLASSW::hInstance` and as the `hInstance` arg of
`CreateWindowExW`. `HINSTANCE` is an alias over `isize` (a pointer).

## What is Send / Sync (so you don't re-derive)

- `FrameBuffer { data: Vec<u8>, width/height/stride/frame_id/timestamp_ns: u32/u64, window_id: u64 }` — `Send`.
- `FrameStore` (`Mutex<Option<FrameBuffer>>` + `AtomicBool`) — `Send + Sync`.
- `Arc<FrameStore>` — `Send + Sync`.
- `HashMap<u64, Arc<FrameStore>>` — `Send`.
- `Arc<Mutex<HashMap<...>>>` — `Send + Sync`.
- `tokio::mpsc::UnboundedSender<T>` — `Send + Sync` when `T: Send`.
  `InputEvent` and `ControlCommand` (your enum of `SetFocus`/`CloseWindow`/
  `ConfigureWindow`/`Shutdown`) are `Send` (plain integers).
- `Arc<Shared>` (with the above fields) — `Send + Sync`.
- `HWND` (`*mut c_void`) is `!Send` in the strict sense (raw pointer), but the
  net task never touches an HWND — it only passes `controller_hwnd` to
  `PostMessageW`, which is thread-safe. Store `controller_hwnd` as `isize` in
  the net task's captured state (cast `HWND` to `isize` before spawning, cast
  back to `HWND` inside `PostMessageW` call). `PostMessageW` takes `HWND` and
  is safe to call from any thread.

## What NOT to do

- Do not create a new `PixmanRenderer` per call (this is server-side; N/A here).
- Do not store an `HWND` directly in a struct that must be `Send` for
  `thread::spawn` — store it as `isize` and cast at the call site.
- Do not blit from the net thread — only the UI thread touches GDI.
- Do not add `#[allow(unsafe_code)]` on individual items; use the single
  module-level `#![allow(unsafe_code)]` at the top of `win.rs`.
- Do not add new `windows-sys` features; the five declared are sufficient.
- Do not re-derive the BGRA byte order — negative `biHeight` + 32bpp
  `BI_RGB` matches pixman readback exactly (decision in
  `lat.md/decisions.md` "BGRA Wire Format").

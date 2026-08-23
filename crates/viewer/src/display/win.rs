//! Win32 display layer (only compiled on Windows).
#![allow(unsafe_code)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use wayland_remote_protocol::{InputEvent, Message, WindowEventKind};

use crate::framebuf::FrameStore;
use crate::input;
use crate::session::ViewerSession;
use crate::window_manager::ViewerWindowManager;

use windows_sys::Win32::Foundation::{HINSTANCE, HMODULE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BeginPaint, ClientToScreen, DIB_RGB_COLORS, EndPaint,
    InvalidateRect, PAINTSTRUCT, SRCCOPY, StretchDIBits,
};
use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateCursor, CreateWindowExW, DefWindowProcW,
    DestroyCursor, DestroyWindow, DispatchMessageW, GWLP_USERDATA, GetClientRect, GetMessageW,
    GetWindowLongPtrW, HCURSOR, HMENU, IDC_ARROW, LoadCursorW, MSG, PostMessageW, PostQuitMessage,
    RegisterClassW, SetCursor, SetCursorPos, ShowCursor, SWP_NOMOVE, SWP_NOZORDER,
    SetWindowLongPtrW, SetWindowPos, SetWindowTextW, TranslateMessage, WM_ACTIVATE, WM_CLOSE,
    WM_DESTROY, WM_ERASEBKGND, WM_KEYDOWN, WM_KEYUP, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN,
    WM_MBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_PAINT, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SIZE,
    WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
};

/// Base for the app's custom window messages.
const WM_USER: u32 = 0x0400;
/// A frame arrived for `wParam` (a `window_id`); the UI thread invalidates it.
const WM_USER_FRAME: u32 = WM_USER + 1;
/// A window lifecycle event; `wParam` = window_id, `lParam` = `Box<WindowEventKind>` raw ptr.
const WM_USER_WIN_EVENT: u32 = WM_USER + 2;
/// Round-trip time updated; `wParam` = rtt milliseconds.
const WM_USER_RTT: u32 = WM_USER + 3;
/// The network task closed; the controller's message loop should quit.
const WM_USER_NET_CLOSED: u32 = WM_USER + 4;
/// A new cursor shape; `wParam` = window_id, `lParam` = `Box<CursorShapeMsg>` raw ptr.
const WM_USER_CURSOR_SHAPE: u32 = WM_USER + 5;
/// Cursor position update; `wParam` = window_id, `lParam` = `Box<CursorMoveMsg>` raw ptr.
const WM_USER_CURSOR_MOVE: u32 = WM_USER + 6;
/// Hide the cursor; `wParam` = window_id.
const WM_USER_CURSOR_HIDE: u32 = WM_USER + 7;

/// Payload posted to the UI thread when the server sends a new cursor sprite.
struct CursorShapeMsg {
    width: u32,
    height: u32,
    hot_x: i32,
    hot_y: i32,
    data: Vec<u8>,
}

/// Payload posted to the UI thread when the cursor position changes.
struct CursorMoveMsg {
    x: f64,
    y: f64,
}

/// Encode a string as a null-terminated UTF-16 buffer for `PCWSTR` args.
fn widen(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}

/// Nanoseconds since the UNIX epoch (0 on error).
fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Build a native 32-bit alpha cursor from a top-down BGRA sprite.
/// `data` must be exactly `width*height*4` bytes, top-down, `[B,G,R,A]`/pixel.
/// Returns a caller-owned HCURSOR (0 on failure); DestroyCursor it before
/// replacing or at shutdown. Run on the UI thread.
fn make_cursor(data: &[u8], width: u32, height: u32, hot_x: i32, hot_y: i32) -> HCURSOR {
    if data.len() != (width * height * 4) as usize {
        return 0 as HCURSOR;
    }
    unsafe {
        CreateCursor(
            0 as HINSTANCE,
            hot_x,
            hot_y,
            width as i32,
            height as i32,
            core::ptr::null(),
            data.as_ptr() as *const core::ffi::c_void,
        )
    }
}

/// Commands the UI thread posts to the network task (focus / close / resize / quit).
#[derive(Debug)]
enum ControlCommand {
    SetFocus(u64),
    CloseWindow(u64),
    ConfigureWindow(u64, u32, u32),
    Shutdown,
}

/// Shared between the UI thread and the network task.
///
/// `Send + Sync`: every field is a `Mutex` or an `UnboundedSender` (both
/// `Send + Sync` because the payload types are plain `Send` integers/enums).
struct Shared {
    frames: Mutex<HashMap<u64, Arc<FrameStore>>>,
    input_tx: tokio::sync::mpsc::UnboundedSender<(u64, InputEvent)>,
    control_tx: tokio::sync::mpsc::UnboundedSender<ControlCommand>,
    manager: Mutex<ViewerWindowManager>,
    focused: Mutex<Option<u64>>,
    cursor: Mutex<Option<isize>>,
    cursor_visible: Mutex<bool>,
}

/// Per-window state stashed on the controller HWND via `GWLP_USERDATA`.
struct UiState {
    shared: Arc<Shared>,
}

/// Per-window state stashed on each child HWND via `GWLP_USERDATA`.
struct ChildState {
    window_id: u64,
    pending_configure: bool,
    client_w: u32,
    client_h: u32,
    shared: Arc<Shared>,
}

/// Entry point for the Windows display: register window classes, create the
/// hidden controller window, run the message loop on the UI thread, and drive
/// the QUIC session on a background tokio thread.
pub fn run(
    addr: std::net::SocketAddr,
    fingerprint: Option<[u8; 32]>,
    insecure: bool,
) -> anyhow::Result<()> {
    let (input_tx, input_rx) = tokio::sync::mpsc::unbounded_channel();
    let (control_tx, control_rx) = tokio::sync::mpsc::unbounded_channel();
    let shared = Arc::new(Shared {
        frames: Mutex::new(HashMap::new()),
        input_tx,
        control_tx,
        manager: Mutex::new(ViewerWindowManager::new()),
        focused: Mutex::new(None),
        cursor: Mutex::new(None),
        cursor_visible: Mutex::new(true),
    });

    // Current process handle; 0 = current process. Returns HMODULE, which is
    // the same raw-pointer type as HINSTANCE.
    let hinst: HMODULE = unsafe { GetModuleHandleW(std::ptr::null()) };

    // Register the controller (hidden) and child (visible) window classes.
    let controller_class = widen("WaylandRemoteController");
    let child_class = widen("WaylandRemoteChild");

    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(controller_proc),
        hInstance: hinst as HINSTANCE,
        hCursor: unsafe { LoadCursorW(hinst as HINSTANCE, IDC_ARROW) },
        lpszClassName: controller_class.as_ptr(),
        ..Default::default()
    };
    assert!(
        unsafe { RegisterClassW(&wc) } != 0,
        "RegisterClassW (controller) failed"
    );

    let wc_child = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(child_proc),
        hInstance: hinst as HINSTANCE,
        hCursor: unsafe { LoadCursorW(hinst as HINSTANCE, IDC_ARROW) },
        lpszClassName: child_class.as_ptr(),
        ..Default::default()
    };
    assert!(
        unsafe { RegisterClassW(&wc_child) } != 0,
        "RegisterClassW (child) failed"
    );

    // Hidden controller window: owns the message loop + the window manager.
    let controller = unsafe {
        CreateWindowExW(
            0,
            controller_class.as_ptr(),
            widen("wayland-remote").as_ptr(),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            0 as HWND,
            0 as HMENU,
            hinst as HINSTANCE,
            core::ptr::null(),
        )
    };
    anyhow::ensure!(!controller.is_null(), "CreateWindowExW (controller) failed");

    let ui_state = Box::new(UiState {
        shared: Arc::clone(&shared),
    });
    unsafe {
        SetWindowLongPtrW(controller, GWLP_USERDATA, Box::into_raw(ui_state) as isize);
    }

    // The net thread runs on its own OS thread, so capture the handle as an
    // `isize` (Send) rather than the `HWND` raw pointer.
    let net_shared = Arc::clone(&shared);
    let controller_handle = controller as isize;
    let net_handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build tokio runtime");
        rt.block_on(net_main(
            addr,
            fingerprint,
            insecure,
            controller_handle,
            net_shared,
            input_rx,
            control_rx,
        ));
    });

    // UI message loop. GetMessageW returns BOOL (i32): 1 = message, 0 = quit,
    // -1 = error; both 0 and -1 end the loop.
    let mut msg = MSG::default();
    loop {
        let r = unsafe { GetMessageW(&mut msg, 0 as HWND, 0, 0) };
        if r <= 0 {
            break;
        }
        unsafe {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    // Free the controller's UiState.
    let state_ptr = unsafe { GetWindowLongPtrW(controller, GWLP_USERDATA) } as *mut UiState;
    if !state_ptr.is_null() {
        let _state = unsafe { Box::from_raw(state_ptr) };
    }

    let _ = net_handle.join();
    Ok(())
}

/// Controller window procedure: receives net→UI `WM_USER_*` messages and routes
/// window lifecycle events through `ViewerWindowManager` / Win32 child windows.
unsafe extern "system" fn controller_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut UiState;
    if state_ptr.is_null() {
        // Early messages (WM_NCCREATE, WM_GETMINMAXINFO, ...) arrive before
        // we stash state. Fall through to the default handler.
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let state = unsafe { &*state_ptr };
    let wid = wparam as u64;

    match msg {
        WM_USER_FRAME => {
            if let Some(child) = state.shared.manager.lock().unwrap().hwnd_for(wid) {
                unsafe {
                    InvalidateRect(child as HWND, core::ptr::null(), 0);
                }
            }
        }
        WM_USER_WIN_EVENT => {
            let event = unsafe { Box::from_raw(lparam as *mut WindowEventKind) };
            {
                let mut manager = state.shared.manager.lock().unwrap();
                manager.handle_event(wid, &event);
            }
            match &*event {
                WindowEventKind::Created {
                    width,
                    height,
                    title,
                } => {
                    let class = widen("WaylandRemoteChild");
                    let title_w = widen(title);
                    let child = unsafe {
                        CreateWindowExW(
                            0,
                            class.as_ptr(),
                            title_w.as_ptr(),
                            WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                            CW_USEDEFAULT,
                            CW_USEDEFAULT,
                            *width as i32,
                            *height as i32,
                            hwnd,
                            0 as HMENU,
                            0 as HINSTANCE,
                            core::ptr::null(),
                        )
                    };
                    if !child.is_null() {
                        let child_state = Box::new(ChildState {
                            window_id: wid,
                            pending_configure: false,
                            client_w: *width,
                            client_h: *height,
                            shared: Arc::clone(&state.shared),
                        });
                        unsafe {
                            SetWindowLongPtrW(
                                child as HWND,
                                GWLP_USERDATA,
                                Box::into_raw(child_state) as isize,
                            );
                        }
                        state
                            .shared
                            .manager
                            .lock()
                            .unwrap()
                            .set_hwnd(wid, child as isize);
                    }
                }
                WindowEventKind::Destroyed => {
                    if let Some(child) = state.shared.manager.lock().unwrap().hwnd_for(wid) {
                        unsafe {
                            DestroyWindow(child as HWND);
                        }
                    }
                    state.shared.manager.lock().unwrap().remove_hwnd(wid);
                    state.shared.frames.lock().unwrap().remove(&wid);
                }
                WindowEventKind::Resized { width, height } => {
                    if let Some(child) = state.shared.manager.lock().unwrap().hwnd_for(wid) {
                        // Clear pending_configure: the server negotiated this
                        // size, so subsequent local WM_SIZE echoes are ours.
                        let child_ptr = unsafe { GetWindowLongPtrW(child as HWND, GWLP_USERDATA) }
                            as *mut ChildState;
                        if !child_ptr.is_null() {
                            unsafe {
                                (*child_ptr).pending_configure = false;
                            }
                        }
                        unsafe {
                            SetWindowPos(
                                child as HWND,
                                0 as HWND,
                                0,
                                0,
                                *width as i32,
                                *height as i32,
                                SWP_NOMOVE | SWP_NOZORDER,
                            );
                        }
                    }
                }
                WindowEventKind::Focused | WindowEventKind::Unfocused => {
                    // No-op: focus state is tracked on the UI thread.
                }
            }
        }
        WM_USER_RTT => {
            let rtt = wparam as u64;
            let focused = *state.shared.focused.lock().unwrap();
            let child = focused.and_then(|f| state.shared.manager.lock().unwrap().hwnd_for(f));
            if let Some(child) = child {
                let title = widen(&format!("wayland-remote — {rtt}ms"));
                unsafe {
                    SetWindowTextW(child as HWND, title.as_ptr());
                }
            }
        }
        WM_USER_CURSOR_SHAPE => {
            let msg = unsafe { Box::from_raw(lparam as *mut CursorShapeMsg) };
            let hcur = make_cursor(&msg.data, msg.width, msg.height, msg.hot_x, msg.hot_y);
            if !hcur.is_null() {
                let old = state.shared.cursor.lock().unwrap().take();
                if let Some(old) = old {
                    unsafe {
                        DestroyCursor(old as HCURSOR);
                    }
                }
                unsafe {
                    SetCursor(hcur);
                }
                *state.shared.cursor.lock().unwrap() = Some(hcur as isize);
                let mut vis = state.shared.cursor_visible.lock().unwrap();
                if !*vis {
                    unsafe {
                        ShowCursor(1);
                    }
                    *vis = true;
                }
            }
        }
        WM_USER_CURSOR_MOVE => {
            let msg = unsafe { Box::from_raw(lparam as *mut CursorMoveMsg) };
            let child = state.shared.manager.lock().unwrap().hwnd_for(wid);
            if let Some(child) = child {
                let child_hwnd = child as HWND;
                let mut pt = POINT { x: 0, y: 0 };
                unsafe {
                    ClientToScreen(child_hwnd, &mut pt);
                }
                let (sx, sy) = {
                    let child_ptr =
                        unsafe { GetWindowLongPtrW(child_hwnd, GWLP_USERDATA) } as *mut ChildState;
                    let (cw, ch) = if child_ptr.is_null() {
                        (0, 0)
                    } else {
                        let cs = unsafe { &*child_ptr };
                        (cs.client_w, cs.client_h)
                    };
                    let frame = state
                        .shared
                        .frames
                        .lock()
                        .unwrap()
                        .get(&wid)
                        .and_then(|s| s.latest());
                    match (frame, (cw, ch)) {
                        (Some(f), (cw, ch))
                            if cw > 0 && ch > 0 && f.width > 0 && f.height > 0 =>
                        {
                            (
                                cw as f64 / f.width as f64,
                                ch as f64 / f.height as f64,
                            )
                        }
                        _ => (1.0, 1.0),
                    }
                };
                let screen_x = pt.x + (msg.x * sx) as i32;
                let screen_y = pt.y + (msg.y * sy) as i32;
                unsafe {
                    SetCursorPos(screen_x, screen_y);
                }
            }
        }
        WM_USER_CURSOR_HIDE => {
            let focused = *state.shared.focused.lock().unwrap();
            if focused == Some(wid) && *state.shared.cursor_visible.lock().unwrap() {
                unsafe {
                    ShowCursor(0);
                }
                *state.shared.cursor_visible.lock().unwrap() = false;
            }
        }
        WM_USER_NET_CLOSED => {
            let old = state.shared.cursor.lock().unwrap().take();
            if let Some(old) = old {
                unsafe {
                    DestroyCursor(old as HCURSOR);
                }
            }
            unsafe {
                PostQuitMessage(0);
            }
        }
        WM_CLOSE => {
            let old = state.shared.cursor.lock().unwrap().take();
            if let Some(old) = old {
                unsafe {
                    DestroyCursor(old as HCURSOR);
                }
            }
            let _ = state.shared.control_tx.send(ControlCommand::Shutdown);
            unsafe {
                PostQuitMessage(0);
            }
        }
        _ => {
            return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
        }
    }
    0
}

/// Child window procedure: blits frames on `WM_PAINT` and forwards input.
unsafe extern "system" fn child_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state_ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut ChildState;
    if state_ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let state = unsafe { &mut *state_ptr };
    let window_id = state.window_id;
    let shared = &state.shared;

    match msg {
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            let hdc = unsafe { BeginPaint(hwnd, &mut ps) };
            if let Some(frame) = shared
                .frames
                .lock()
                .unwrap()
                .get(&window_id)
                .and_then(|s| s.latest())
            {
                let mut r = RECT::default();
                unsafe {
                    GetClientRect(hwnd, &mut r);
                }
                let cw = r.right - r.left;
                let ch = r.bottom - r.top;
                if cw > 0 && ch > 0 && frame.width > 0 && frame.height > 0 {
                    let mut info = BITMAPINFO::default();
                    info.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
                    info.bmiHeader.biWidth = frame.width as i32;
                    info.bmiHeader.biHeight = -(frame.height as i32);
                    info.bmiHeader.biPlanes = 1;
                    info.bmiHeader.biBitCount = 32;
                    info.bmiHeader.biCompression = BI_RGB;

                    // Stride guard: BI_RGB 32bpp assumes contiguous rows
                    // (width * 4). Pack into a contiguous buffer when the server
                    // padded the rows.
                    let tight = frame.stride as usize == frame.width as usize * 4;
                    let _packed: Vec<u8>;
                    let bits: *const core::ffi::c_void;
                    if tight {
                        bits = frame.data.as_ptr() as *const core::ffi::c_void;
                    } else {
                        let row = frame.width as usize * 4;
                        let mut p = vec![0u8; row * frame.height as usize];
                        for y in 0..frame.height as usize {
                            let src = y * frame.stride as usize;
                            p[y * row..y * row + row].copy_from_slice(&frame.data[src..src + row]);
                        }
                        _packed = p;
                        bits = _packed.as_ptr() as *const core::ffi::c_void;
                    }
                    unsafe {
                        StretchDIBits(
                            hdc,
                            0,
                            0,
                            cw,
                            ch,
                            0,
                            0,
                            frame.width as i32,
                            frame.height as i32,
                            bits,
                            &info,
                            DIB_RGB_COLORS,
                            SRCCOPY,
                        );
                    }
                }
            }
            unsafe {
                EndPaint(hwnd, &ps);
            }
            0
        }
        WM_ERASEBKGND => 1,
        WM_KEYDOWN | WM_KEYUP => {
            let (scancode, is_extended) = input::extract_scancode(lparam as usize);
            let pressed = msg == WM_KEYDOWN;
            let ev = input::key_event(scancode, is_extended, pressed);
            let _ = shared.input_tx.send((window_id, ev));
            0
        }
        WM_MOUSEMOVE => {
            let (x, y) = input::pointer_move(lparam as i32);
            let (sx, sy) = {
                let frames = shared.frames.lock().unwrap();
                match (
                    frames.get(&window_id).and_then(|s| s.size()),
                    (state.client_w, state.client_h),
                ) {
                    (Some((fw, fh)), (cw, ch)) if cw > 0 && ch > 0 && fw > 0 && fh > 0 => {
                        (x * (fw as f64 / cw as f64), y * (fh as f64 / ch as f64))
                    }
                    _ => (x, y),
                }
            };
            let ev = InputEvent::PointerMove { x: sx, y: sy };
            let _ = shared.input_tx.send((window_id, ev));
            0
        }
        WM_LBUTTONDOWN | WM_LBUTTONUP | WM_RBUTTONDOWN | WM_RBUTTONUP | WM_MBUTTONDOWN
        | WM_MBUTTONUP => {
            if let Some((button, btn_state)) = input::pointer_button(msg, wparam) {
                let ev = InputEvent::PointerButton {
                    button,
                    state: btn_state,
                };
                let _ = shared.input_tx.send((window_id, ev));
            }
            0
        }
        WM_MOUSEWHEEL => {
            let (dx, dy) = input::scroll(msg, wparam);
            let ev = InputEvent::Axis { dx, dy };
            let _ = shared.input_tx.send((window_id, ev));
            0
        }
        WM_ACTIVATE => {
            // Low word: 0 = inactive, 1 = active, 2 = click-active.
            let activate = (wparam & 0xFFFF) as u32;
            if activate == 1 || activate == 2 {
                *shared.focused.lock().unwrap() = Some(window_id);
                let _ = shared.control_tx.send(ControlCommand::SetFocus(window_id));
            }
            0
        }
        WM_SIZE => {
            state.client_w = (lparam & 0xFFFF) as u32;
            state.client_h = ((lparam >> 16) & 0xFFFF) as u32;
            if !state.pending_configure && state.client_w > 0 && state.client_h > 0 {
                state.pending_configure = true;
                let _ = shared.control_tx.send(ControlCommand::ConfigureWindow(
                    window_id,
                    state.client_w,
                    state.client_h,
                ));
            }
            0
        }
        WM_CLOSE => {
            let _ = shared
                .control_tx
                .send(ControlCommand::CloseWindow(window_id));
            0
        }
        WM_DESTROY => 0,
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Background network task: owns the `ViewerSession`, runs the frame-receive
/// loop on a spawned tokio task (cloned `Connection`), and the control loop
/// (`&mut session`) via `tokio::select!`.
async fn net_main(
    addr: std::net::SocketAddr,
    fingerprint: Option<[u8; 32]>,
    insecure: bool,
    controller_isize: isize,
    shared: Arc<Shared>,
    mut input_rx: tokio::sync::mpsc::UnboundedReceiver<(u64, InputEvent)>,
    mut control_rx: tokio::sync::mpsc::UnboundedReceiver<ControlCommand>,
) {
    let controller = controller_isize as HWND;
    let mut session = match ViewerSession::connect(addr, fingerprint, insecure).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("viewer: connect failed: {e:?}");
            unsafe {
                PostMessageW(controller, WM_USER_NET_CLOSED, 0, 0);
            }
            return;
        }
    };

    // Frame-receive task: owns a cheap `Connection` clone, swaps frames into
    // `shared.frames`, posts `WM_USER_FRAME`. Exits when the stream errors.
    //
    // The spawned future must be `Send`, so it captures the `isize` handle
    // (never the `HWND` raw pointer) and casts at the call site.
    let conn = session.connection();
    let frame_shared = Arc::clone(&shared);
    let frame_controller = controller_isize;
    tokio::spawn(async move {
        loop {
            match ViewerSession::read_frame(&conn).await {
                Ok(frame) => {
                    let wid = frame.window_id;
                    frame_shared
                        .frames
                        .lock()
                        .unwrap()
                        .entry(wid)
                        .or_insert_with(|| Arc::new(FrameStore::new()))
                        .swap(frame);
                    unsafe {
                        PostMessageW(frame_controller as HWND, WM_USER_FRAME, wid as usize, 0);
                    }
                }
                Err(e) => {
                    eprintln!("viewer: frame stream ended: {e:?}");
                    unsafe {
                        PostMessageW(frame_controller as HWND, WM_USER_NET_CLOSED, 0, 0);
                    }
                    break;
                }
            }
        }
    });

    // Control loop: owns `&mut session`. All `select!` branches borrow
    // `&mut session` only — no conflict, because frame reception lives in the
    // spawned task above (a separate `Connection`).
    let mut ping = tokio::time::interval(Duration::from_millis(500));
    ping.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    let mut pending_ping: Option<u64> = None;

    loop {
        tokio::select! {
            Some((wid, ev)) = input_rx.recv() => {
                if session.send_input(wid, ev).await.is_err() {
                    unsafe {
                        PostMessageW(controller, WM_USER_NET_CLOSED, 0, 0);
                    }
                    break;
                }
            }
            Some(cmd) = control_rx.recv() => {
                match cmd {
                    ControlCommand::SetFocus(w) => {
                        if session
                            .send_control(&Message::SetFocus { window_id: w })
                            .await
                            .is_err()
                        {
                            unsafe {
                                PostMessageW(controller, WM_USER_NET_CLOSED, 0, 0);
                            }
                            break;
                        }
                    }
                    ControlCommand::CloseWindow(w) => {
                        if session
                            .send_control(&Message::CloseWindow { window_id: w })
                            .await
                            .is_err()
                        {
                            unsafe {
                                PostMessageW(controller, WM_USER_NET_CLOSED, 0, 0);
                            }
                            break;
                        }
                    }
                    ControlCommand::ConfigureWindow(w, cw, ch) => {
                        if session
                            .send_control(&Message::ConfigureWindow {
                                window_id: w,
                                width: cw,
                                height: ch,
                            })
                            .await
                            .is_err()
                        {
                            unsafe {
                                PostMessageW(controller, WM_USER_NET_CLOSED, 0, 0);
                            }
                            break;
                        }
                    }
                    ControlCommand::Shutdown => {
                        session.close();
                        unsafe {
                            PostMessageW(controller, WM_USER_NET_CLOSED, 0, 0);
                        }
                        break;
                    }
                }
            }
            _ = ping.tick() => {
                let ts = now_ns();
                pending_ping = Some(ts);
                let _ = session
                    .send_control(&Message::Ping { timestamp_ns: ts })
                    .await;
            }
            maybe = async {
                match tokio::time::timeout(
                    Duration::from_millis(50),
                    session.read_control(),
                )
                .await {
                    Ok(Ok(msg)) => Some(msg),
                    _ => None,
                }
            } => {
                if let Some(Message::WindowEvent {
                    window_id,
                    event,
                }) = maybe
                {
                    let raw = Box::into_raw(Box::new(event)) as isize;
                    unsafe {
                        PostMessageW(controller, WM_USER_WIN_EVENT, window_id as usize, raw);
                    }
                } else if let Some(Message::Pong { timestamp_ns }) = maybe {
                    if Some(timestamp_ns) == pending_ping {
                        let rtt = now_ns().saturating_sub(timestamp_ns) / 1_000_000;
                        unsafe {
                            PostMessageW(controller, WM_USER_RTT, rtt as usize, 0);
                        }
                    }
                } else if let Some(Message::Ping { timestamp_ns }) = maybe {
                    let _ = session
                        .send_control(&Message::Pong { timestamp_ns })
                        .await;
                } else if let Some(Message::CursorShape {
                    window_id,
                    width,
                    height,
                    hot_x,
                    hot_y,
                    data,
                }) = maybe
                {
                    let raw = Box::into_raw(Box::new(CursorShapeMsg {
                        width,
                        height,
                        hot_x,
                        hot_y,
                        data,
                    })) as isize;
                    unsafe {
                        PostMessageW(controller, WM_USER_CURSOR_SHAPE, window_id as usize, raw);
                    }
                } else if let Some(Message::CursorMove { window_id, x, y }) = maybe {
                    let raw = Box::into_raw(Box::new(CursorMoveMsg { x, y })) as isize;
                    unsafe {
                        PostMessageW(controller, WM_USER_CURSOR_MOVE, window_id as usize, raw);
                    }
                } else if let Some(Message::CursorHide { window_id }) = maybe {
                    unsafe {
                        PostMessageW(controller, WM_USER_CURSOR_HIDE, window_id as usize, 0);
                    }
                }
            }
        }
    }
}

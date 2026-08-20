//! Window manager: tracks xdg toplevels, assigns window_ids, manages focus.
#![allow(clippy::collapsible_if)]

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel;
use smithay::wayland::shell::xdg::ToplevelSurface;
use wayland_remote_protocol::WindowEventKind;
use wayland_server::Resource;
use wayland_server::backend::ObjectId;
use wayland_server::protocol::wl_surface::WlSurface;

/// A tracked toplevel window.
#[derive(Debug)]
pub struct Window {
    /// Stable id assigned by the window manager; travels on the wire in
    /// `WindowEvent`s and `FrameHeader::window_id`.
    pub window_id: u64,
    /// The xdg toplevel handle (cloned: it is a cheap resource reference).
    pub toplevel: ToplevelSurface,
    /// Object id of the underlying `wl_surface` (key into the surface map).
    pub surface_id: ObjectId,
    /// Last client-set title (empty until the client sends one).
    pub title: String,
    /// Last committed buffer width.
    pub width: u32,
    /// Last committed buffer height.
    pub height: u32,
    /// True once the window has acked its initial configure and committed a
    /// buffer — only then is it visible to viewers.
    pub mapped: bool,
    /// True once the client has acked at least one configure.
    pub acked: bool,
}

/// Manages the lifecycle of xdg toplevels.
///
/// A toplevel is only "mapped" (visible to viewers) after the client has
/// acked its initial configure and committed a buffer — the
/// initial-configure trap of the xdg-shell protocol. Mapped windows are
/// announced to viewers with a `Created` window event; the focused window
/// carries the xdg `Activated` state.
#[derive(Debug, Default)]
pub struct WindowManager {
    windows: HashMap<u64, Window>,
    surface_to_window: HashMap<ObjectId, u64>,
    next_id: AtomicU64,
    focused: Option<u64>,
    /// Pending window events to send to viewers (drained by the network task).
    pub pending_events: Vec<(u64, WindowEventKind)>,
}

impl WindowManager {
    /// Create an empty window manager.
    #[must_use]
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            surface_to_window: HashMap::new(),
            next_id: AtomicU64::new(1),
            focused: None,
            pending_events: Vec::new(),
        }
    }

    /// Register a new toplevel (called from the `new_toplevel` handler).
    ///
    /// The window is not mapped yet: mapping happens on the first commit
    /// after the client has acked its initial configure.
    pub fn register(&mut self, toplevel: ToplevelSurface) {
        let surface = toplevel.wl_surface();
        let id = surface.id();
        let window_id = self.next_id.fetch_add(1, Ordering::Relaxed);
        self.windows.insert(
            window_id,
            Window {
                window_id,
                toplevel,
                surface_id: id.clone(),
                title: String::new(),
                width: 0,
                height: 0,
                mapped: false,
                acked: false,
            },
        );
        self.surface_to_window.insert(id, window_id);
    }

    /// Mark a window as having acked its first configure.
    pub fn mark_acked(&mut self, surface: &WlSurface) {
        let id = surface.id();
        if let Some(&wid) = self.surface_to_window.get(&id) {
            if let Some(win) = self.windows.get_mut(&wid) {
                win.acked = true;
            }
        }
    }

    /// Called from `CompositorHandler::commit` when a surface commits a buffer.
    ///
    /// If this is the first post-ack commit, the window becomes "mapped" and
    /// a `Created` event is queued. The first mapped window is focused
    /// (xdg `Activated` state sent via a follow-up configure).
    pub fn on_commit(&mut self, surface: &WlSurface, width: u32, height: u32) {
        let id = surface.id();
        if let Some(&wid) = self.surface_to_window.get(&id) {
            if let Some(win) = self.windows.get_mut(&wid) {
                // An already-mapped window re-committed at a different size is
                // resized: update the stored size and queue a Resized event.
                // Unchanged sizes emit nothing.
                let resized = win.mapped && (win.width, win.height) != (width, height);
                win.width = width;
                win.height = height;
                if resized {
                    self.pending_events
                        .push((wid, WindowEventKind::Resized { width, height }));
                }
                if win.acked && !win.mapped {
                    win.mapped = true;
                    // Set initial focus if none
                    if self.focused.is_none() {
                        self.focused = Some(wid);
                        win.toplevel.with_pending_state(|s| {
                            s.states.set(xdg_toplevel::State::Activated);
                        });
                        win.toplevel.send_configure();
                    }
                    self.pending_events.push((
                        wid,
                        WindowEventKind::Created {
                            width,
                            height,
                            title: win.title.clone(),
                        },
                    ));
                }
            }
        }
    }

    /// Update the stored title (called when the client sets a title).
    pub fn set_title(&mut self, surface: &WlSurface, title: String) {
        let id = surface.id();
        if let Some(&wid) = self.surface_to_window.get(&id) {
            if let Some(win) = self.windows.get_mut(&wid) {
                win.title = title;
            }
        }
    }

    /// Destroy a window (called from the `toplevel_destroyed` handler).
    ///
    /// Emits a `Destroyed` event; if the focused window was destroyed,
    /// focus moves to the next tracked window (activation configure sent).
    pub fn destroy(&mut self, toplevel: &ToplevelSurface) {
        let surface = toplevel.wl_surface();
        let id = surface.id();
        if let Some(wid) = self.surface_to_window.remove(&id) {
            self.windows.remove(&wid);
            self.pending_events.push((wid, WindowEventKind::Destroyed));
            if self.focused == Some(wid) {
                self.focused = self.windows.keys().next().copied();
                // Activate the new focused window
                if let Some(new_focused) = self.focused {
                    if let Some(win) = self.windows.get(&new_focused) {
                        win.toplevel.with_pending_state(|s| {
                            s.states.set(xdg_toplevel::State::Activated);
                        });
                        win.toplevel.send_configure();
                    }
                }
            }
        }
    }

    /// Focus a specific window (from a `SetFocus` message).
    ///
    /// Deactivates the old focus and activates the new one via pending
    /// state + configure. Unknown window ids are ignored.
    pub fn set_focus(&mut self, window_id: u64) {
        if !self.windows.contains_key(&window_id) {
            return;
        }
        // Deactivate old focus
        if let Some(old) = self.focused
            && old != window_id
        {
            if let Some(win) = self.windows.get(&old) {
                win.toplevel.with_pending_state(|s| {
                    s.states.unset(xdg_toplevel::State::Activated);
                });
                win.toplevel.send_configure();
            }
        }
        // Activate new
        self.focused = Some(window_id);
        if let Some(win) = self.windows.get(&window_id) {
            win.toplevel.with_pending_state(|s| {
                s.states.set(xdg_toplevel::State::Activated);
            });
            win.toplevel.send_configure();
        }
    }

    /// Request a window to close (from a `CloseWindow` message).
    ///
    /// Sends the xdg `close` event; the client decides whether to comply
    /// (protocol-wise it may ignore it, like a native window close).
    pub fn close_window(&mut self, window_id: u64) {
        if let Some(win) = self.windows.get(&window_id) {
            win.toplevel.send_close();
        }
    }

    /// Configure a window to a new size (from a `ConfigureWindow` message).
    pub fn configure_window(&mut self, window_id: u64, width: u32, height: u32) {
        if let Some(win) = self.windows.get(&window_id) {
            win.toplevel.with_pending_state(|state| {
                state.size = Some((width as i32, height as i32).into());
            });
            win.toplevel.send_configure();
        }
    }

    /// Get the focused surface for keyboard focus.
    #[must_use]
    pub fn focused_surface(&self) -> Option<&WlSurface> {
        self.focused
            .and_then(|id| self.windows.get(&id))
            .map(|w| w.toplevel.wl_surface())
    }

    /// The focused window id, if any.
    #[must_use]
    pub fn focused(&self) -> Option<u64> {
        self.focused
    }

    /// Drain pending window events for the network.
    pub fn drain_events(&mut self) -> Vec<(u64, WindowEventKind)> {
        std::mem::take(&mut self.pending_events)
    }

    /// Number of mapped windows.
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.windows.values().filter(|w| w.mapped).count()
    }

    /// The ids of all mapped windows, in no particular order.
    #[must_use]
    pub fn mapped_windows(&self) -> Vec<u64> {
        self.windows
            .values()
            .filter(|w| w.mapped)
            .map(|w| w.window_id)
            .collect()
    }

    /// The object id of the window's underlying `wl_surface`, if the window
    /// is tracked (key into `State::surfaces`).
    #[must_use]
    pub fn surface_id_for(&self, window_id: u64) -> Option<&ObjectId> {
        self.windows.get(&window_id).map(|w| &w.surface_id)
    }

    /// The `WlSurface` backing a window, for input focus injection.
    #[must_use]
    pub fn surface_for(&self, window_id: u64) -> Option<&WlSurface> {
        self.windows
            .get(&window_id)
            .map(|w| w.toplevel.wl_surface())
    }

    /// The committed (width, height) of a mapped window, if it is tracked.
    #[must_use]
    pub fn window_size(&self, window_id: u64) -> Option<(u32, u32)> {
        self.windows
            .get(&window_id)
            .filter(|w| w.mapped)
            .map(|w| (w.width, w.height))
    }
}

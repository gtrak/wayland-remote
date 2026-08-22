//! Legacy wl_shell global (hand-rolled — smithay 0.7 has no legacy shell).
//!
//! Maps legacy toplevels into the WindowManager the same way xdg toplevels
//! are mapped: register on `set_toplevel`, send a configure size hint, map
//! on the client's first buffer commit. Interactive move/resize grabs,
//! popups, and transients are ignored.

use wayland_server::backend::ClientId;
use wayland_server::protocol::wl_shell::{self, WlShell};
use wayland_server::protocol::wl_shell_surface::{self, Resize, WlShellSurface};
use wayland_server::protocol::wl_surface::WlSurface;
use wayland_server::{Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New, Resource};

use crate::state::State;

/// Per-shell-surface user data: the underlying wl_surface.
#[derive(Debug, Clone)]
pub struct WlShellSurfaceData {
    /// The `wl_surface` this shell surface is bound to.
    pub surface: WlSurface,
}

/// The legacy wl_shell global state.
#[derive(Debug)]
pub struct WlShellState;

impl WlShellState {
    /// Create the `wl_shell` global (version 1).
    #[must_use]
    pub fn new(display: &DisplayHandle) -> Self {
        display.create_global::<State, WlShell, ()>(1, ());
        Self
    }
}

impl GlobalDispatch<WlShell, (), State> for WlShellState {
    fn bind(
        _state: &mut State,
        _dh: &DisplayHandle,
        _client: &Client,
        resource: New<WlShell>,
        _global_data: &(),
        data_init: &mut DataInit<'_, State>,
    ) {
        data_init.init(resource, ());
    }
}

impl Dispatch<WlShell, (), State> for WlShellState {
    fn request(
        _state: &mut State,
        _client: &Client,
        resource: &WlShell,
        request: wl_shell::Request,
        _data: &(),
        _dh: &DisplayHandle,
        data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            wl_shell::Request::GetShellSurface { id, surface } => {
                // A wl_surface may only take one role. Taking the shell
                // surface role here rejects double-role clients (e.g. an
                // xdg toplevel reusing the surface) with the protocol error.
                if smithay::wayland::compositor::give_role(&surface, "shell_surface").is_err() {
                    resource.post_error(
                        wl_shell::Error::Role,
                        "wl_surface already has another role",
                    );
                    return;
                }
                data_init.init(id, WlShellSurfaceData { surface });
            }
            _ => {}
        }
    }
}

impl Dispatch<WlShellSurface, WlShellSurfaceData, State> for WlShellState {
    fn request(
        state: &mut State,
        _client: &Client,
        resource: &WlShellSurface,
        request: wl_shell_surface::Request,
        data: &WlShellSurfaceData,
        _dh: &DisplayHandle,
        _data_init: &mut DataInit<'_, State>,
    ) {
        match request {
            // Toplevel, fullscreen, and maximized all map the same way here:
            // register the window and hint the output size. Legacy wl_shell
            // has no ack_configure, so the first buffer commit maps it.
            wl_shell_surface::Request::SetToplevel
            | wl_shell_surface::Request::SetFullscreen { .. }
            | wl_shell_surface::Request::SetMaximized { .. } => {
                state
                    .window_manager
                    .register_legacy_shell(data.surface.clone(), resource.clone());
                resource.configure(
                    Resize::empty(),
                    state.config.width as i32,
                    state.config.height as i32,
                );
            }
            wl_shell_surface::Request::SetTitle { title } => {
                state.window_manager.set_title(&data.surface, title);
            }
            // Interactive move/resize grabs, popups, transients, and class
            // are not supported by the headless compositor.
            wl_shell_surface::Request::Pong { .. }
            | wl_shell_surface::Request::Move { .. }
            | wl_shell_surface::Request::Resize { .. }
            | wl_shell_surface::Request::SetTransient { .. }
            | wl_shell_surface::Request::SetPopup { .. }
            | wl_shell_surface::Request::SetClass { .. } => {}
            _ => {}
        }
    }

    fn destroyed(
        state: &mut State,
        _client: ClientId,
        _resource: &WlShellSurface,
        data: &WlShellSurfaceData,
    ) {
        // Fires on client disconnect; a no-op if the surface destruction
        // already removed the window.
        state.window_manager.destroy(&data.surface);
    }
}

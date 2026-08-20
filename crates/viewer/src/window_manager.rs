//! Viewer-side window management: tracks windows by window_id.
//!
//! In the real Win32 viewer (display/win.rs), each window becomes an HWND.
//! In headless mode (Linux CI), this module just tracks state.

use std::collections::HashMap;
use wayland_remote_protocol::WindowEventKind;

#[derive(Debug, Clone)]
pub struct ViewerWindow {
    pub window_id: u64,
    pub width: u32,
    pub height: u32,
    pub title: String,
}

pub struct ViewerWindowManager {
    windows: HashMap<u64, ViewerWindow>,
    #[cfg(windows)]
    hwnds: HashMap<u64, isize>,
}

impl ViewerWindowManager {
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            #[cfg(windows)]
            hwnds: HashMap::new(),
        }
    }

    /// Record the HWND backing a window (Windows only). Mutated from
    /// `display/win.rs`, not from `handle_event` (which stays platform-neutral).
    #[cfg(windows)]
    pub fn set_hwnd(&mut self, window_id: u64, hwnd: isize) {
        self.hwnds.insert(window_id, hwnd);
    }

    /// Look up the HWND backing a window (Windows only).
    #[cfg(windows)]
    pub fn hwnd_for(&self, window_id: u64) -> Option<isize> {
        self.hwnds.get(&window_id).copied()
    }

    /// Forget the HWND backing a window (Windows only).
    #[cfg(windows)]
    pub fn remove_hwnd(&mut self, window_id: u64) {
        self.hwnds.remove(&window_id);
    }

    pub fn handle_event(&mut self, window_id: u64, event: &WindowEventKind) {
        match event {
            WindowEventKind::Created {
                width,
                height,
                title,
            } => {
                self.windows.insert(
                    window_id,
                    ViewerWindow {
                        window_id,
                        width: *width,
                        height: *height,
                        title: title.clone(),
                    },
                );
            }
            WindowEventKind::Destroyed => {
                self.windows.remove(&window_id);
            }
            WindowEventKind::Resized { width, height } => {
                if let Some(win) = self.windows.get_mut(&window_id) {
                    win.width = *width;
                    win.height = *height;
                }
            }
            WindowEventKind::Focused | WindowEventKind::Unfocused => {
                // Track focus state if needed later
            }
        }
    }

    pub fn window_count(&self) -> usize {
        self.windows.len()
    }
    pub fn get(&self, id: u64) -> Option<&ViewerWindow> {
        self.windows.get(&id)
    }
    pub fn window_ids(&self) -> Vec<u64> {
        self.windows.keys().copied().collect()
    }
}

impl Default for ViewerWindowManager {
    fn default() -> Self {
        Self::new()
    }
}

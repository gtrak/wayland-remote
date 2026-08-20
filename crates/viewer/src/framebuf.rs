//! Frame buffer storage for the viewer.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// A decoded frame ready for display.
#[derive(Debug, Clone)]
pub struct FrameBuffer {
    pub data: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub stride: u32,
    pub frame_id: u64,
    pub timestamp_ns: u64,
    pub window_id: u64,
}

/// Thread-safe frame store with double buffering: the network thread
/// swaps in new frames; the display thread borrows the latest.
pub struct FrameStore {
    front: Mutex<Option<FrameBuffer>>,
    has_new: AtomicBool,
}

impl FrameStore {
    pub fn new() -> Self {
        Self {
            front: Mutex::new(None),
            has_new: AtomicBool::new(false),
        }
    }

    /// Store a new frame (called from the network thread).
    pub fn swap(&self, frame: FrameBuffer) {
        let mut front = self.front.lock().unwrap();
        *front = Some(frame);
        self.has_new.store(true, Ordering::Relaxed);
    }

    /// Borrow the latest frame if a new one arrived since the last call.
    pub fn borrow(&self) -> Option<FrameBuffer> {
        if !self.has_new.swap(false, Ordering::Relaxed) {
            return None;
        }
        self.front.lock().unwrap().clone()
    }

    /// Clone the front frame without clearing the new-flag.
    ///
    /// Used by the Win32 `WM_PAINT` handler to re-read the current frame after
    /// a resize or uncover (the flag must survive so a later `borrow` still
    /// reports a pending update).
    pub fn latest(&self) -> Option<FrameBuffer> {
        self.front.lock().unwrap().clone()
    }

    /// The dimensions of the front frame, if any frame has been stored.
    ///
    /// Used by input translation to scale client-area pointer coordinates up to
    /// surface coordinates.
    pub fn size(&self) -> Option<(u32, u32)> {
        self.front
            .lock()
            .unwrap()
            .as_ref()
            .map(|f| (f.width, f.height))
    }
}

impl Default for FrameStore {
    fn default() -> Self {
        Self::new()
    }
}

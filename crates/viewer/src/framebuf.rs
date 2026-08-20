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
}

impl Default for FrameStore {
    fn default() -> Self {
        Self::new()
    }
}

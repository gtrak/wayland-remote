//! Scancode to Linux keycode translation.
//!
//! Windows Set-1 scancodes ARE Linux evdev keycodes (e.g. scancode 0x1E = 30 =
//! KEY_A). Smithay's KeyboardHandle internally adds 8 for xkb, so we pass the
//! raw scancode as the keycode. Extended keys (sent by the viewer with a 0x100
//! offset) use a lookup table.

/// Translate a viewer scancode to a Linux evdev keycode.
///
/// Non-extended scancodes (< 0x100): keycode = scancode (identity — Set-1
/// scancodes and evdev keycodes share the same numbering).
/// Extended scancodes (>= 0x100): strip 0x100, look up in the extended table.
/// Returns None for unmapped extended keys.
pub fn scancode_to_keycode(scancode: u16) -> Option<u32> {
    if scancode < 0x100 {
        Some(scancode as u32)
    } else {
        extended_keycode((scancode & 0xFF) as u8)
    }
}

/// Extended scancode → Linux keycode mapping (Set 1 extended keys).
fn extended_keycode(ext: u8) -> Option<u32> {
    Some(match ext {
        0x1C => 96,  // KP Enter
        0x1D => 97,  // Right Ctrl
        0x35 => 98,  // KP Divide
        0x38 => 100, // Right Alt
        0x47 => 102, // Home
        0x48 => 103, // Up
        0x49 => 104, // Page Up
        0x4B => 105, // Left
        0x4D => 106, // Right
        0x4F => 107, // End
        0x50 => 108, // Down
        0x51 => 109, // Page Down
        0x52 => 110, // Insert
        0x53 => 111, // Delete
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::scancode_to_keycode;

    /// Non-extended scancode 0x1E (A key) → evdev KEY_A (30).
    #[test]
    // @lat: [[tests#Input Injection#Non-extended scancode identity]]
    fn non_extended_scancode_identity() {
        assert_eq!(scancode_to_keycode(0x1E), Some(30));
    }

    /// Extended Left arrow (0x100 | 0x4B) maps to KEY_LEFT (105).
    #[test]
    // @lat: [[tests#Input Injection#Extended key mapping]]
    fn extended_left_arrow_maps_to_key_left() {
        assert_eq!(scancode_to_keycode(0x100 | 0x4B), Some(105));
    }

    /// Unmapped extended scancodes yield None (dropped upstream, never panic).
    #[test]
    // @lat: [[tests#Input Injection#Unknown scancode dropped]]
    fn unmapped_extended_scancode_returns_none() {
        assert_eq!(scancode_to_keycode(0x100 | 0x99), None);
    }
}

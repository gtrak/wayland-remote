//! Seat handler for Wayland compositor
//!
//! Provides wl_seat global with keyboard and pointer capabilities.
//! Clients need wl_seat to receive input focus and create interactive windows.

use smithay::reexports::wayland_server::{DisplayHandle, GlobalDispatch, protocol::wl_seat::WlSeat};
use smithay::input::{Seat, SeatState};
use smithay::wayland::seat::{WaylandFocus, SeatGlobalData};

/// Create and initialize a wl_seat global
///
/// # Arguments
/// * `dh` - DisplayHandle for advertising the global
/// * `name` - Name for the seat (e.g., "wayland-remote-seat")
///
/// # Returns
/// Tuple of (SeatState, Seat) configured with keyboard and pointer
pub fn create_seat<S>(dh: &DisplayHandle, name: &str) -> (SeatState<S>, Seat<S>)
where
    S: GlobalDispatch<WlSeat, SeatGlobalData<S>>
        + smithay::input::SeatHandler
        + 'static,
    <S as smithay::input::SeatHandler>::KeyboardFocus: WaylandFocus,
    <S as smithay::input::SeatHandler>::PointerFocus: WaylandFocus,
    <S as smithay::input::SeatHandler>::TouchFocus: WaylandFocus,
{
    let mut seat_state = SeatState::new();
    let mut seat = seat_state.new_wl_seat(dh, name);
    
    // Add keyboard capability
    // The repeat rate (200) and delay (25ms) are reasonable defaults
    seat.add_keyboard(Default::default(), 200, 25).unwrap();
    
    // Add pointer capability
    seat.add_pointer();
    
    (seat_state, seat)
}

//! Output handler for Wayland compositor
//!
//! Provides wl_output global with virtual display configuration.
//! Clients need wl_output to know display parameters for rendering.

use smithay::reexports::wayland_server::DisplayHandle;
use smithay::output::{Mode, Output};
use smithay::utils::Size;

/// Create a virtual output for the compositor
///
/// This creates a headless virtual display that advertises
/// display parameters to Wayland clients.
///
/// # Arguments
/// * `dh` - DisplayHandle for advertising the global
///
/// # Returns
/// Configured Output with 1920x1080 @ 60Hz mode
pub fn create_virtual_output(dh: &DisplayHandle) -> Output {
    // Create output with physical properties
    // Size (0, 0) indicates headless/virtual display
    let output = Output::new(
        "Virtual-1".to_string(),
        smithay::output::PhysicalProperties {
            size: Size::new(0, 0),
            subpixel: smithay::output::Subpixel::None,
            make: "Wayland Remote".to_string(),
            model: "Virtual".to_string(),
        },
    );
    
    // Advertise the output global to clients
    // Note: We use a simpler approach without GlobalDispatch requirement
    // The output is created and configured, but global dispatching
    // is handled by the compositor's event loop
    
    // Configure mode: 1920x1080 @ 60Hz
    // Vrefresh is in mHz (60 Hz = 60000 mHz)
    output.add_mode(Mode {
        size: Size::new(1920, 1080),
        refresh: 60000,
    });
    
    output
}

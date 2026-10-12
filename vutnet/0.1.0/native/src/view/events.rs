//! Native builds do not dispatch browser DOM events. These symbols only satisfy
//! the View event methods when their Vut modules are linked into a native app.

#[unsafe(no_mangle)]
pub extern "C" fn vut_browser_event_prevent_default(_control_id: i32) {}

#[unsafe(no_mangle)]
pub extern "C" fn vut_browser_event_stop_propagation(_control_id: i32) {}

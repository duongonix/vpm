//! SSR binding evaluation has no browser dependency collector.
#![allow(unsafe_code, reason = "C symbols shared with the browser binding host")]

#[unsafe(no_mangle)]
pub extern "C" fn vutnet_view_scope_begin_v1() {}

#[unsafe(no_mangle)]
pub extern "C" fn vutnet_view_scope_end_v1() {}

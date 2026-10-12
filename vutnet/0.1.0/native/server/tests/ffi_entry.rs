use vutnet_server_native::ffi::vutnet_server_serve_fixed_v1;

#[test]
fn rejects_ports_outside_the_valid_range() {
    assert_eq!(vutnet_server_serve_fixed_v1(70_000, 200), -2);
    assert_eq!(vutnet_server_serve_fixed_v1(-1, 200), -2);
}

#[test]
fn rejects_invalid_final_statuses() {
    // Valid port, invalid status: validation must happen before binding, so
    // these calls return immediately instead of starting a server.
    assert_eq!(vutnet_server_serve_fixed_v1(0, 99), -1);
    assert_eq!(vutnet_server_serve_fixed_v1(0, 100), -1);
    assert_eq!(vutnet_server_serve_fixed_v1(0, 600), -1);
    assert_eq!(vutnet_server_serve_fixed_v1(0, -5), -1);
}

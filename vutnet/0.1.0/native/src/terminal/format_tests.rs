use std::time::Duration;

use http::StatusCode;

use super::*;

#[test]
fn startup_output_is_compact_and_uses_actual_bound_port() {
    let rendered = ready(
        "0.1.0",
        "127.0.0.1:43821".parse().expect("socket address"),
        Duration::from_millis(184),
        None,
        None,
        false,
    );

    assert_eq!(
        rendered,
        "  VUTNET  v0.1.0\n\n  ✓ Server ready in 184ms\n\n  ➜ Local     http://localhost:43821\n"
    );
}

#[test]
fn request_columns_remain_aligned_and_long_paths_are_bounded() {
    let get = request("GET", "/", StatusCode::OK, Duration::from_millis(3), false);
    let post = request(
        "POST",
        "/a/longer/path",
        StatusCode::CREATED,
        Duration::from_millis(8),
        false,
    );
    let get_status = get.find("  200  ").expect("GET status column");
    let post_status = post.find("  201  ").expect("POST status column");
    assert_eq!(get_status, post_status);
    assert!(get.contains("GET      /"));
    assert!(post.contains("POST     /a/longer/path"));

    let long = request(
        "DELETE",
        "/a/very/long/request/path/that/should/be/truncated/cleanly",
        StatusCode::NO_CONTENT,
        Duration::from_micros(300),
        false,
    );
    let truncated_path = long.split_whitespace().nth(1).expect("request path column");
    assert_eq!(truncated_path.chars().count(), PATH_WIDTH);
    assert!(truncated_path.ends_with('…'));
    assert!(long.ends_with("  204  <1ms\n"));
}

#[test]
fn color_is_added_only_when_requested_and_uses_semantic_styles() {
    let plain = request("GET", "/", StatusCode::OK, Duration::from_millis(1), false);
    assert!(!plain.contains('\u{1b}'));

    let colored = request(
        "POST",
        "/login",
        StatusCode::UNAUTHORIZED,
        Duration::from_millis(2),
        true,
    );
    assert!(colored.contains("\u{1b}[32mPOST"));
    assert!(colored.contains("\u{1b}[33m401"));

    for (method, code, method_color, status_color) in [
        ("GET", StatusCode::OK, "36", "32"),
        ("PUT", StatusCode::FOUND, "33", "36"),
        ("PATCH", StatusCode::NOT_FOUND, "33", "33"),
        ("DELETE", StatusCode::INTERNAL_SERVER_ERROR, "31", "31"),
    ] {
        let line = request(method, "/", code, Duration::from_millis(1), true);
        assert!(line.contains(&format!("\u{1b}[{method_color}m{method}")));
        assert!(line.contains(&format!("\u{1b}[{status_color}m{}", code.as_u16())));
    }

    let ready_line = ready(
        "0.1.0",
        "127.0.0.1:3000".parse().expect("socket address"),
        Duration::from_millis(1),
        None,
        None,
        true,
    );
    assert!(ready_line.contains("\u{1b}[35mVUTNET"));
    assert!(ready_line.contains("\u{1b}[4m\u{1b}[36mhttp://localhost:3000"));

    let dev = ready(
        "0.1.0",
        "127.0.0.1:3000".parse().expect("socket address"),
        Duration::from_millis(1),
        Some("development"),
        Some("src/**/*.vut"),
        false,
    );
    assert!(dev.contains("Mode      development"));
    assert!(dev.contains("Watching  src/**/*.vut"));
}

#[test]
fn adaptive_stream_strips_ansi_for_non_terminal_output() {
    let mut output = anstream::AutoStream::auto(Vec::new());
    std::io::Write::write_all(&mut output, "\u{1b}[31mred\u{1b}[0m".as_bytes())
        .expect("write styled text");
    let output = String::from_utf8(output.into_inner()).expect("utf-8 output");
    assert_eq!(output, "red");
}

#[test]
fn log_text_cannot_inject_terminal_control_sequences() {
    let line = request(
        "GET",
        "/safe\u{1b}[31m",
        StatusCode::OK,
        Duration::from_millis(1),
        false,
    );
    assert!(line.contains(r"/safe\u{1b}[31m"));
    assert!(!line.contains('\u{1b}'));
}

#[test]
fn error_and_shutdown_have_clear_plain_text_forms() {
    assert_eq!(
        error("Could not bind the server", false),
        "  ✗ ERROR\n\n  Could not bind the server\n"
    );
    assert_eq!(stopped(false), "  ✓ Server stopped\n");
}

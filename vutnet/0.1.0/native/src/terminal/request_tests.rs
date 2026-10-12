use http::StatusCode;

use super::{RequestSource, should_log_request};

#[test]
fn application_requests_are_logged_by_default() {
    assert!(should_log_request(
        RequestSource::Application,
        StatusCode::OK,
        false,
    ));
    assert!(should_log_request(
        RequestSource::Application,
        StatusCode::CREATED,
        false,
    ));
}

#[test]
fn successful_static_css_js_image_and_font_responses_are_suppressed() {
    for asset in [
        "/assets/generated/tailwind.css",
        "/assets/app.js",
        "/assets/logo.svg",
        "/assets/fonts/site.woff2",
        "/favicon.ico",
    ] {
        // Static responses share one handler classification, independent of
        // extension or MIME type.
        assert!(
            !should_log_request(RequestSource::StaticFiles, StatusCode::OK, false),
            "successful static response for {asset} should be hidden"
        );
    }

    for status in [
        StatusCode::CREATED,
        StatusCode::FOUND,
        StatusCode::NOT_MODIFIED,
    ] {
        assert!(
            !should_log_request(RequestSource::StaticFiles, status, false),
            "successful static response {status} should be hidden"
        );
    }
}

#[test]
fn static_errors_remain_visible() {
    for status in [
        StatusCode::BAD_REQUEST,
        StatusCode::NOT_FOUND,
        StatusCode::INTERNAL_SERVER_ERROR,
    ] {
        assert!(
            should_log_request(RequestSource::StaticFiles, status, false),
            "static error {status} should be logged"
        );
    }
}

#[test]
fn verbose_logs_successful_static_responses() {
    assert!(should_log_request(
        RequestSource::StaticFiles,
        StatusCode::OK,
        true,
    ));
}

#[test]
fn application_routes_with_asset_looking_paths_are_not_filtered() {
    // The path is intentionally not an input to the policy. `routing` marks a
    // response as static only when StaticFiles::response handled the request.
    assert!(
        should_log_request(RequestSource::Application, StatusCode::OK, false,),
        "a Router response for /api/theme.css must still be logged"
    );
}

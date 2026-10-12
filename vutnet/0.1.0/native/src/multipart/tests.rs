use http::{HeaderMap, HeaderValue, header::CONTENT_TYPE};
use multer::bytes::Bytes;

use super::{MAX_PARTS, parse_form, valid_boundary};
use crate::multipart::{Form, Issue};

#[test]
fn boundary_is_bounded_and_excludes_controls() {
    assert!(valid_boundary("AaB03x"));
    assert!(valid_boundary("quoted boundary"));
    for value in ["", "trailing ", "newline\r\n", "invalid@", &"x".repeat(71)] {
        assert!(!valid_boundary(value));
    }
}

#[tokio::test]
async fn accepts_part_bound_and_rejects_excess_without_partial_snapshot() {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("multipart/form-data; boundary=b"),
    );
    let part = b"--b\r\nContent-Disposition: form-data; name=\"tag\"\r\n\r\na\r\n";
    let mut body = part.repeat(MAX_PARTS);
    body.extend_from_slice(b"--b--\r\n");
    let accepted = parse_form(&headers, Bytes::from(body)).await.unwrap();
    assert_eq!(accepted.fields.len(), MAX_PARTS);
    let mut body = part.repeat(MAX_PARTS + 1);
    body.extend_from_slice(b"--b--\r\n");
    let issue = parse_form(&headers, Bytes::from(body)).await.err().unwrap();
    assert_eq!(issue, Issue::TooManyParts);
    let failed = Form::failed(issue);
    assert!(failed.fields.is_empty());
    assert!(failed.files.is_empty());
    assert_eq!(failed.issue, Some(Issue::TooManyParts));
}

#[tokio::test]
async fn follows_multer_header_normalization_and_rejects_invalid_part_content_type() {
    let mut headers = HeaderMap::new();
    headers.insert(
        CONTENT_TYPE,
        HeaderValue::from_static("multipart/form-data; boundary=b"),
    );
    // Multer normalizes repeated part header names to the last wire value.
    // Both validation and field access operate on that same parsed value.
    let body = "--b\r\nContent-Disposition: form-data; name=\"a\"\r\nContent-Disposition: form-data; name=\"b\"\r\n\r\ncontent\r\n--b--\r\n";
    let form = parse_form(&headers, Bytes::from_static(body.as_bytes()))
        .await
        .unwrap();
    assert_eq!(form.fields, [("b".to_owned(), "content".to_owned())]);
    let body = "--b\r\nContent-Disposition: form-data; name=\"a\"; filename=\"f\"\r\nContent-Type: bad mime\r\n\r\ncontent\r\n--b--\r\n";
    assert_eq!(
        parse_form(&headers, Bytes::from_static(body.as_bytes()))
            .await
            .err(),
        Some(Issue::Malformed)
    );
}

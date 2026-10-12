use bytes::Bytes;
use http::{HeaderMap, HeaderName, HeaderValue, Response, StatusCode, header::CONTENT_TYPE};
use http_body_util::{BodyExt, Full};

use crate::streaming::HttpBody;

/// `Content-Type` used by [`ServerResponse::text`].
pub const TEXT_CONTENT_TYPE: &str = "text/plain; charset=utf-8";
/// `Content-Type` used by [`ServerResponse::bytes`].
pub const BINARY_CONTENT_TYPE: &str = "application/octet-stream";

/// An owned server-side response transport value.
///
/// This is the native counterpart of a future Vutnet `Response`. It owns all
/// status/header/body data and converts to a Hyper response exactly once.
#[derive(Clone, Debug)]
pub struct ServerResponse {
    status: StatusCode,
    headers: HeaderMap,
    body: Bytes,
}

impl ServerResponse {
    /// Creates an empty response with the given status.
    #[must_use]
    pub fn new(status: StatusCode) -> Self {
        Self {
            status,
            headers: HeaderMap::new(),
            body: Bytes::new(),
        }
    }

    /// Creates a UTF-8 text response and sets the default text `Content-Type`.
    #[must_use]
    pub fn text(status: StatusCode, body: impl Into<Bytes>) -> Self {
        Self::with_content_type(status, body.into(), TEXT_CONTENT_TYPE)
    }

    /// Creates a binary response and sets the default octet-stream `Content-Type`.
    #[must_use]
    pub fn bytes(status: StatusCode, body: impl Into<Bytes>) -> Self {
        Self::with_content_type(status, body.into(), BINARY_CONTENT_TYPE)
    }

    /// Adds or replaces a response header.
    #[must_use]
    pub fn with_header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.headers.insert(name, value);
        self
    }

    /// The response status code.
    #[must_use]
    pub fn status(&self) -> StatusCode {
        self.status
    }

    /// The response headers.
    #[must_use]
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// The response body.
    #[must_use]
    pub fn body(&self) -> &Bytes {
        &self.body
    }

    fn with_content_type(status: StatusCode, body: Bytes, content_type: &'static str) -> Self {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static(content_type));
        Self {
            status,
            headers,
            body,
        }
    }

    /// Consumes the value into a Hyper response. Called exactly once.
    pub(crate) fn into_http(self) -> Response<HttpBody> {
        let body = Full::new(self.body)
            .map_err(|never| match never {})
            .boxed_unsync();
        let mut response = Response::new(body);
        *response.status_mut() = self.status;
        *response.headers_mut() = self.headers;
        response
    }
}

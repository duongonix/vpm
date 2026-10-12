use std::net::SocketAddr;

use bytes::Bytes;
use http::{HeaderMap, Method};

/// An owned server-side request transport value.
///
/// The value owns every field; no borrowed pointer into the Hyper request or a
/// stack frame escapes the connection task. This is the native counterpart of a
/// future Vutnet `Context` snapshot.
#[derive(Clone, Debug)]
pub struct ServerRequest {
    method: Method,
    path: String,
    query: Option<String>,
    headers: HeaderMap,
    body: Bytes,
    peer: SocketAddr,
    request_id: u64,
}

impl ServerRequest {
    pub(crate) fn from_parts(
        parts: http::request::Parts,
        body: Bytes,
        peer: SocketAddr,
        request_id: u64,
    ) -> Self {
        let path = if parts.uri.path().is_empty() {
            "/".to_owned()
        } else {
            parts.uri.path().to_owned()
        };
        Self {
            method: parts.method,
            path,
            query: parts.uri.query().map(str::to_owned),
            headers: parts.headers,
            body,
            peer,
            request_id,
        }
    }

    /// The request method.
    #[must_use]
    pub fn method(&self) -> &Method {
        &self.method
    }

    /// The request path, excluding query and fragment, percent-encoded as sent.
    #[must_use]
    pub fn path(&self) -> &str {
        &self.path
    }

    /// The raw query string, if the request URI had one.
    #[must_use]
    pub fn query(&self) -> Option<&str> {
        self.query.as_deref()
    }

    /// All request headers.
    #[must_use]
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// The owned request body snapshot.
    #[must_use]
    pub fn body(&self) -> &Bytes {
        &self.body
    }

    /// The direct socket peer address.
    #[must_use]
    pub fn peer(&self) -> SocketAddr {
        self.peer
    }

    /// A process-lifetime-unique request identifier for tracing.
    #[must_use]
    pub fn request_id(&self) -> u64 {
        self.request_id
    }

    /// Consumes the request and returns its owned body.
    #[must_use]
    pub fn into_body(self) -> Bytes {
        self.body
    }
}

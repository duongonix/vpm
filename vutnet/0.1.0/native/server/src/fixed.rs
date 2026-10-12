use bytes::Bytes;
use http::StatusCode;

use crate::handler::{Handler, HandlerFuture};
use crate::request::ServerRequest;
use crate::response::ServerResponse;

/// A handler that answers every request with the same fixed response.
///
/// This is the first-phase native proof handler used by the scalar C entrypoint
/// in [`crate::ffi`]. It is not part of the Vutnet application API.
#[derive(Clone, Debug)]
pub struct FixedHandler {
    response: ServerResponse,
}

impl FixedHandler {
    /// Wraps a prebuilt response.
    #[must_use]
    pub fn new(response: ServerResponse) -> Self {
        Self { response }
    }

    /// Answers every request with a fixed text response.
    #[must_use]
    pub fn text(status: StatusCode, body: impl Into<Bytes>) -> Self {
        Self::new(ServerResponse::text(status, body))
    }
}

impl Handler for FixedHandler {
    fn handle(&self, _request: ServerRequest) -> HandlerFuture {
        let response = self.response.clone();
        Box::pin(async move { response })
    }
}

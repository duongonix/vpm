use std::fmt;

use crate::{ServerResponse, streaming::HttpBody};
use http::{Response, StatusCode};

/// Errors produced while binding or accepting connections.
///
/// Request-level failures that must become an HTTP response (oversized body,
/// malformed framing) are handled by the connection layer as deterministic
/// responses instead of surfacing here.
#[derive(Debug)]
pub enum ServerError {
    /// The listener could not bind to the configured address.
    Bind { address: String, message: String },
    /// The OS accept loop failed with a fatal error.
    Accept { message: String },
}

impl ServerError {
    pub(crate) fn bind(address: String, error: &std::io::Error) -> Self {
        Self::Bind {
            address,
            message: error.to_string(),
        }
    }

    pub(crate) fn accept(error: &std::io::Error) -> Self {
        Self::Accept {
            message: error.to_string(),
        }
    }
}

impl fmt::Display for ServerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bind { address, message } => {
                write!(
                    formatter,
                    "failed to bind HTTP server to {address}: {message}"
                )
            }
            Self::Accept { message } => {
                write!(formatter, "HTTP server accept failed: {message}")
            }
        }
    }
}

impl std::error::Error for ServerError {}

/// Builds a deterministic plain-text error response.
pub(crate) fn error_response(status: StatusCode, message: &'static str) -> Response<HttpBody> {
    ServerResponse::text(status, message).into_http()
}

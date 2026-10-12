use std::net::SocketAddr;

/// Default bounded request-body limit (1 MiB).
///
/// The limit is finite on purpose: an unbounded body would let a remote peer
/// grow server memory without limit before any handler runs.
pub const DEFAULT_MAX_BODY_BYTES: usize = 1024 * 1024;

/// Listener and per-request limits for a [`crate::Server`].
#[derive(Clone, Debug)]
pub struct ServerConfig {
    address: SocketAddr,
    max_body_bytes: usize,
}

impl ServerConfig {
    /// Creates a configuration that binds `address` with the default body limit.
    #[must_use]
    pub fn bind(address: SocketAddr) -> Self {
        Self {
            address,
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
        }
    }

    /// Sets the maximum number of request body bytes accepted per request.
    ///
    /// A request that declares or streams more than this limit is rejected with
    /// `413 Payload Too Large` before any handler runs.
    #[must_use]
    pub fn with_max_body_bytes(mut self, max_body_bytes: usize) -> Self {
        self.max_body_bytes = max_body_bytes;
        self
    }

    /// The address the server binds to.
    #[must_use]
    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// The configured request-body limit.
    #[must_use]
    pub fn max_body_bytes(&self) -> usize {
        self.max_body_bytes
    }
}

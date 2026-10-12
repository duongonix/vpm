use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::time::Duration;

use tokio::net::TcpListener;

use crate::config::ServerConfig;
use crate::connection;
use crate::error::ServerError;
use crate::fixed::FixedHandler;
use crate::handler::Handler;
use crate::lifecycle::ConnectionTracker;
use crate::response::ServerResponse;
use crate::shutdown::{ShutdownHandle, ShutdownState};

/// Consecutive accept failures tolerated before the server reports a fatal
/// accept error instead of retrying forever.
const MAX_CONSECUTIVE_ACCEPT_ERRORS: u32 = 16;

/// A bound HTTP server listener.
///
/// Bind with [`Server::bind`], then drive it with one of the `serve_*`
/// methods. The server owns the listener and every accepted connection until
/// the server future resolves.
pub struct Server {
    listener: TcpListener,
    config: ServerConfig,
}

impl Server {
    /// Binds the listener described by `config`.
    ///
    /// # Errors
    /// Returns [`ServerError::Bind`] when the address cannot be bound.
    pub async fn bind(config: ServerConfig) -> Result<Self, ServerError> {
        let listener = TcpListener::bind(config.address())
            .await
            .map_err(|error| ServerError::bind(config.address().to_string(), &error))?;
        Ok(Self { listener, config })
    }

    /// The actual bound address, useful when binding port `0` in tests.
    ///
    /// # Errors
    /// Returns the OS error if the address can no longer be queried.
    pub fn local_addr(&self) -> std::io::Result<SocketAddr> {
        self.listener.local_addr()
    }

    /// The configuration this server was bound with.
    #[must_use]
    pub fn config(&self) -> &ServerConfig {
        &self.config
    }

    /// Serves `handler` until the future is cancelled or dropped.
    ///
    /// # Errors
    /// Returns [`ServerError::Accept`] if the accept loop fails repeatedly.
    pub async fn serve_with<H>(self, handler: H) -> Result<(), ServerError>
    where
        H: Handler,
    {
        self.serve_inner(handler, Arc::new(ShutdownState::default()))
            .await
    }

    /// Serves `handler` until `shutdown` is triggered, then drains and closes
    /// every active connection before resolving.
    ///
    /// # Errors
    /// Returns [`ServerError::Accept`] if the accept loop fails repeatedly.
    pub async fn serve_with_shutdown<H>(
        self,
        handler: H,
        shutdown: ShutdownHandle,
    ) -> Result<(), ServerError>
    where
        H: Handler,
    {
        self.serve_inner(handler, shutdown.state()).await
    }

    /// Answers every request with `response` until `shutdown` is triggered.
    ///
    /// # Errors
    /// Returns [`ServerError::Accept`] if the accept loop fails repeatedly.
    pub async fn serve_fixed(
        self,
        response: ServerResponse,
        shutdown: ShutdownHandle,
    ) -> Result<(), ServerError> {
        self.serve_with_shutdown(FixedHandler::new(response), shutdown)
            .await
    }

    /// Serves `handler` until the process receives an interrupt, then shuts
    /// down cleanly. Used by the blocking scalar C entrypoint.
    ///
    /// # Errors
    /// Returns [`ServerError::Accept`] if the accept loop fails repeatedly.
    pub async fn serve_until_interrupt<H>(self, handler: H) -> Result<(), ServerError>
    where
        H: Handler,
    {
        let shutdown = ShutdownHandle::new();
        let trigger = shutdown.clone();
        tokio::spawn(async move {
            if tokio::signal::ctrl_c().await.is_ok() {
                trigger.shutdown();
            }
        });
        self.serve_inner(handler, shutdown.state()).await
    }

    async fn serve_inner<H>(
        self,
        handler: H,
        shutdown: Arc<ShutdownState>,
    ) -> Result<(), ServerError>
    where
        H: Handler,
    {
        let handler = Arc::new(handler);
        let request_ids = Arc::new(AtomicU64::new(0));
        let tracker = ConnectionTracker::new();
        let mut consecutive_accept_errors = 0_u32;

        loop {
            tokio::select! {
                () = shutdown.wait() => break,
                accepted = self.listener.accept() => {
                    match accepted {
                        Ok((stream, peer)) => {
                            consecutive_accept_errors = 0;
                            let guard = tracker.track();
                            let handler = Arc::clone(&handler);
                            let config = self.config.clone();
                            let request_ids = Arc::clone(&request_ids);
                            let state = Arc::clone(&shutdown);
                            tokio::spawn(async move {
                                let _guard = guard;
                                connection::serve(stream, peer, handler, config, request_ids, state).await;
                            });
                        }
                        Err(error) => {
                            if shutdown.is_requested() {
                                break;
                            }
                            // Transient accept failures (descriptor exhaustion,
                            // aborted handshakes) must not kill a running server,
                            // but a permanently failing listener must not spin
                            // silently either.
                            consecutive_accept_errors += 1;
                            if consecutive_accept_errors > MAX_CONSECUTIVE_ACCEPT_ERRORS {
                                return Err(ServerError::accept(&error));
                            }
                            tokio::time::sleep(Duration::from_millis(10)).await;
                        }
                    }
                }
            }
        }

        // Wait for every spawned connection to finish its graceful close.
        tracker.wait_idle().await;
        Ok(())
    }
}

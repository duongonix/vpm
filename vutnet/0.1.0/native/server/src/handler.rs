use std::future::Future;
use std::pin::Pin;

use crate::NativeResponse;
use crate::request::ServerRequest;
use crate::response::ServerResponse;

/// A boxed handler future. Boxing keeps [`Handler`] object-safe and lets the
/// server stay generic over any `async` implementation.
pub type HandlerFuture = Pin<Box<dyn Future<Output = ServerResponse> + Send + 'static>>;

/// Move-only native response future, including file streaming.
pub type NativeResponseFuture = Pin<Box<dyn Future<Output = NativeResponse> + Send + 'static>>;

/// The native request-in / response-out boundary.
///
/// The server core has no knowledge of Vut. A later phase adapts the Vutnet
/// controller callback ABI to this trait; nothing here refers to controllers,
/// routers, middleware, or the Vutnet `Context`/`Response` model.
pub trait Handler: Send + Sync + 'static {
    /// Handles one fully-read request and returns an owned response.
    fn handle(&self, request: ServerRequest) -> HandlerFuture;

    /// Extends the buffered boundary without changing existing handlers.
    fn handle_response(&self, request: ServerRequest) -> NativeResponseFuture {
        let result = self.handle(request);
        Box::pin(async move { NativeResponse::Buffered(result.await) })
    }
}

impl<F, Fut> Handler for F
where
    F: Fn(ServerRequest) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = ServerResponse> + Send + 'static,
{
    fn handle(&self, request: ServerRequest) -> HandlerFuture {
        Box::pin(self(request))
    }
}

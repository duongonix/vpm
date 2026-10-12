use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use http::{Request, Response, StatusCode};
use hyper::body::{Body as _, Incoming};
use hyper::service::service_fn;
use hyper_util::rt::{TokioExecutor, TokioIo};
use hyper_util::server::conn::auto;
use tokio::net::TcpStream;

use crate::body::{BodyError, collect_bounded};
use crate::config::ServerConfig;
use crate::error::error_response;
use crate::handler::Handler;
use crate::request::ServerRequest;
use crate::shutdown::ShutdownState;
use crate::streaming::HttpBody;

/// Serves every request on one accepted TCP connection until the peer closes,
/// the connection errors, or shutdown is requested.
pub(crate) async fn serve<H>(
    stream: TcpStream,
    peer: SocketAddr,
    handler: Arc<H>,
    config: ServerConfig,
    request_ids: Arc<AtomicU64>,
    shutdown: Arc<ShutdownState>,
) where
    H: Handler,
{
    // Latency over coalescing for HTTP request/response traffic.
    let _ = stream.set_nodelay(true);

    let service = service_fn(move |request: Request<Incoming>| {
        let handler = Arc::clone(&handler);
        let config = config.clone();
        let request_ids = Arc::clone(&request_ids);
        async move {
            // `service_fn` requires a `Result`; this service is infallible
            // because every failure is already a deterministic response.
            Ok::<_, std::convert::Infallible>(
                respond(request, handler, config, request_ids, peer).await,
            )
        }
    });

    let mut builder = auto::Builder::new(TokioExecutor::new());
    builder.http1().max_buf_size(64 * 1024);
    let connection = builder.serve_connection_with_upgrades(TokioIo::new(stream), service);
    tokio::pin!(connection);

    tokio::select! {
        result = &mut connection => {
            let _ = result;
        }
        () = shutdown.wait() => {
            // Finish the in-flight request, then close keep-alive connections.
            connection.as_mut().graceful_shutdown();
            let _ = connection.await;
        }
    }
}

async fn respond<H>(
    request: Request<Incoming>,
    handler: Arc<H>,
    config: ServerConfig,
    request_ids: Arc<AtomicU64>,
    peer: SocketAddr,
) -> Response<HttpBody>
where
    H: Handler,
{
    let limit = config.max_body_bytes();

    // Reject a declared oversized body before reading any of it.
    if let Some(length) = request.body().size_hint().exact()
        && length > limit as u64
    {
        return error_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large");
    }

    let (parts, body) = request.into_parts();
    let body = match collect_bounded(body, limit).await {
        Ok(body) => body,
        Err(BodyError::TooLarge) => {
            return error_response(StatusCode::PAYLOAD_TOO_LARGE, "request body too large");
        }
        Err(BodyError::Read) => {
            return error_response(StatusCode::BAD_REQUEST, "invalid request body");
        }
    };

    let request_id = request_ids.fetch_add(1, Ordering::Relaxed);
    let request = ServerRequest::from_parts(parts, body, peer, request_id);
    handler.handle_response(request).await.into_http()
}

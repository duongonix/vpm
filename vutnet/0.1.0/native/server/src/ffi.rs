//! Scalar C ABI entrypoints for Vutnet's legacy server proof facade.
//!
//! Only the Public Native ABI v1 scalar surface is used here (`i32` in, `i32`
//! out). The rich request/response transport — and therefore Vutnet controller
//! integration — belongs to the public Native Callback ABI phase; this module
//! deliberately does not touch private runtime handles.

use std::net::{Ipv4Addr, SocketAddr};

use http::StatusCode;
use tokio::runtime::Builder;

use crate::config::ServerConfig;
use crate::fixed::FixedHandler;
use crate::server::Server;

/// Native call returned after a clean shutdown.
const STATUS_OK: i32 = 0;
/// The requested response status is not a valid final HTTP status.
const STATUS_INVALID_STATUS: i32 = -1;
/// The requested port is outside `0..=65535`.
const STATUS_INVALID_PORT: i32 = -2;
/// The Tokio runtime could not be created.
const STATUS_RUNTIME: i32 = -3;
/// The listener could not be bound.
const STATUS_BIND: i32 = -4;

/// Blocking HTTP-server proof entrypoint.
///
/// Binds `127.0.0.1:<port>`, answers every request with `status` and a fixed
/// UTF-8 text body, and shuts down cleanly on a process interrupt. Returns
/// `0` on clean shutdown or a negative status code on a setup failure.
///
/// This is the phase-2 link target for the Vut `server.serve_fixed` wrapper. It
/// uses only boundary-safe scalars; it is not the Vutnet application API.
#[allow(
    unsafe_code,
    reason = "the C ABI entrypoint needs the no_mangle attribute"
)]
#[unsafe(no_mangle)]
pub extern "C" fn vutnet_server_serve_fixed_v1(port: i32, status: i32) -> i32 {
    let Ok(port) = u16::try_from(port) else {
        return STATUS_INVALID_PORT;
    };
    let Some(status) = u16::try_from(status)
        .ok()
        .and_then(|code| StatusCode::from_u16(code).ok())
    else {
        return STATUS_INVALID_STATUS;
    };
    if status.is_informational() || status.as_u16() > 599 {
        return STATUS_INVALID_STATUS;
    }

    let Ok(runtime) = Builder::new_multi_thread().enable_all().build() else {
        return STATUS_RUNTIME;
    };

    runtime.block_on(async move {
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let Ok(server) = Server::bind(ServerConfig::bind(address)).await else {
            return STATUS_BIND;
        };
        let handler = FixedHandler::text(status, "Hello from the Vut server");
        if server.serve_until_interrupt(handler).await.is_err() {
            return STATUS_RUNTIME;
        }
        STATUS_OK
    })
}

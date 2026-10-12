//! Internal HTTP server engine for the Vutnet VPM package.
//!
//! This crate owns low-level HTTP transport only. It knows nothing about Vut
//! routers, controllers, middleware, or the Vutnet `Context` / `Response`
//! model. The sibling Vutnet native adapter maps the public callback ABI onto
//! this crate's [`Handler`] boundary.
//!
//! Layer:
//!
//! ```text
//! Vutnet VPM package
//!   ├── Vut API + native adapter
//!   └── this internal server crate
//!         ↓
//! Hyper + Tokio
//!         ↓
//! OS / network
//! ```
//!
//! The HTTP protocol and the async I/O are delegated to mature crates. This
//! crate only owns connection/listener lifecycle, bounded request bodies, the
//! owned request/response transport representation, deterministic error
//! responses and clean shutdown.

mod body;
mod config;
mod connection;
mod error;
pub mod ffi;
mod fixed;
mod handler;
mod lifecycle;
mod request;
mod response;
mod server;
mod shutdown;
mod streaming;

pub use config::{DEFAULT_MAX_BODY_BYTES, ServerConfig};
pub use error::ServerError;
pub use fixed::FixedHandler;
pub use handler::{Handler, HandlerFuture, NativeResponseFuture};
pub use request::ServerRequest;
pub use response::ServerResponse;
pub use server::Server;
pub use shutdown::ShutdownHandle;
pub use streaming::{FileResponse, NativeResponse};

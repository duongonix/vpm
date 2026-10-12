# Vutnet — Internal HTTP Server Engine

## 1. Scope

`vutnet-server-native` is an internal Cargo crate at
`vpm-package/vutnet/native/server`. It owns low-level HTTP transport below the
Vutnet native adapter, but ships and versions with the single Vutnet VPM
package:

~~~text
Vut application
        ↓
Vutnet VPM package
  ├── Vut API and native adapter
  └── internal vutnet-server-native Cargo crate
        ↓
Hyper + Tokio
        ↓
OS / network
~~~

The crate owns:

- `Server` (bind/listen);
- connection and listener lifecycle;
- bounded request-body transport;
- owned request transport;
- owned response transport;
- handler boundary;
- server errors;
- clean shutdown.

The crate must NOT own:

- Router DSL;
- Controller / RouterFactory;
- Vutnet Context;
- middleware;
- view/render;
- application framework behavior.

The crate is not a VPM package or public Vut namespace. It does not depend on
a separate `net` package; Tokio/Hyper provide networking.

## 2. Legacy scalar Vut proof entrypoint

The package retains a small `vutnet.server` scalar facade for the Phase 2
server proof and native link regression. It is not the framework's application
server API; the production App/Router/Context/Response path is owned by Vutnet.

~~~vut
import vutnet.server as server

fn serve_fixed(port: i32, status: i32) -> i32
~~~

- Binds `127.0.0.1:<port>`.
- Answers every request with HTTP `status` (100–599, informational excluded)
  and the fixed UTF-8 text body `Hello from the Vut server`.
- Sets `Content-Type: text/plain; charset=utf-8` and Hyper sets
  `Content-Length`.
- Shuts down cleanly on a process interrupt and returns `0`.
- Negative return codes: `-1` invalid status, `-2` invalid port, `-3` runtime
  failure, `-4` bind failure.

This is a native-core proof entrypoint, not the Vutnet application API. It uses
only Public Native ABI v1 scalars (`i32` in/out) and does not reference any
private Runtime Handle ABI symbol.

The `vutnet.server.serve_fixed` Vut entrypoint is retained only as a scalar
link proof. Its Rust `Server`, `Handler`, `ServerRequest` and `ServerResponse`
are internal types consumed directly by the Vutnet adapter; they are not exposed
as a rich Vut server API. The adapter and engine are built together from the
single Vutnet VPM package and communicate inside one native Cargo build.

## 3. Native module architecture

The internal `vutnet-server-native` crate is a member of the standalone
Cargo workspace rooted at `native/Cargo.toml`. Its dependencies are pinned by
`native/Cargo.lock`, separate from the compiler workspace lockfile:

~~~text
vpm-package/vutnet/native/server/src/
├── lib.rs          crate facade, re-exports
├── server.rs       Server: bind, serve_*, local_addr
├── connection.rs   per-connection Hyper service, body limit, error responses
├── config.rs       ServerConfig, body limit
├── request.rs      ServerRequest (owned transport value)
├── response.rs     ServerResponse (owned transport value)
├── streaming.rs    NativeResponse / FileResponse → streamed Hyper body
├── handler.rs      Handler trait + closure blanket impl
├── fixed.rs        FixedHandler (phase-2 fixed response)
├── body.rs         bounded body collection
├── error.rs        ServerError + deterministic error responses
├── lifecycle.rs    ConnectionTracker / RAII guard
├── shutdown.rs     ShutdownState / ShutdownHandle
└── ffi.rs          scalar C ABI entrypoint
~~~

Rust crates used: Tokio (runtime, net, sync, time, signal), Hyper (HTTP/1 and
HTTP/2 server protocol), hyper-util (Tokio adapters and the HTTP/1+HTTP/2 auto
connection builder), http (methods/headers/status/request/response primitives),
http-body-util (body extraction and `Full` responses) and bytes (owned body
buffers). No HTTP parser, TCP stack or async executor is written by hand.

Static integration adds native Rust `Handler.handle_response` with a default
implementation wrapping the existing `handle` result in `NativeResponse::Buffered`.
`NativeResponse::File(FileResponse)` consumes an already-opened regular file,
length and validated Content-Type. ReaderStream/Tokio fs streams 64 KiB chunks;
the file closes when the body completes or drops. Buffered ServerResponse and
FixedHandler remain cloneable and unchanged at their construction boundary.
The HTTP body is an owned fallible boxed body shared by buffered/file responses.
Confinement, MIME selection, HEAD and static routing policy belong to Vutnet.
This is an additive native transport capability, not a public Vut streaming API.

## 4. HTTP request lifecycle

~~~text
TCP accept (Server)
    ↓
spawn connection task, track it
    ↓
Hyper parses the request (HTTP/1 or HTTP/2 via auto builder)
    ↓
declared Content-Length over limit?  → 413, stop
    ↓
bounded body collection              → 413 / 400 on limit or read failure
    ↓
build owned ServerRequest (method, path, query, headers, body, peer, request id)
    ↓
Handler::handle(ServerRequest) -> ServerResponse
    ↓
convert ServerResponse to Hyper response exactly once
    ↓
client
~~~

Malformed framing (bad request line/headers) is rejected by Hyper with a
deterministic 400 or a connection close. Oversized bodies are rejected before
the handler runs.

## 5. Ownership and cleanup model

- The `Server` owns the bound listener. Dropping the server drops the listener.
- Each accepted connection is a spawned task tracked by `ConnectionTracker`.
  An RAII guard decrements the counter on every exit path, including panic and
  cancellation.
- `ServerRequest` owns method, path, query, headers, body, peer and request id.
  No borrowed pointer into the Hyper request or a stack frame escapes the
  connection task.
- `ServerResponse` owns status, headers and body and is consumed exactly once by
  `into_http`.
- The request body buffer never exceeds `ServerConfig::max_body_bytes`; it is
  checked before every append.
- Shutdown: `ShutdownHandle::shutdown` stops the accept loop, every connection
  calls `graceful_shutdown`, and the server future resolves only after the
  connection tracker reaches zero.
- Errors are values (`ServerError` / deterministic HTTP responses); no unwind is
  allowed to cross the C ABI. Pull-in of a panic-across-C boundary is avoided by
  keeping the FFI entrypoint a plain `extern "C"` function that returns an
  integer status.

## 6. Definition of Done (phase 2)

Proven by the crate's own integration tests over a real socket with a real raw
HTTP/1.1 client:

- bind/listen on a real socket;
- real HTTP request received and a fixed real response returned;
- status, `Content-Type` and `Content-Length` correct;
- bounded request body;
- malformed and oversized requests handled deterministically;
- concurrent requests;
- listener and connection cleanup on shutdown;
- clean shutdown of idle keep-alive connections;
- repeated server lifecycle without leaks.

The Cargo engine remains independently testable without a Vut Controller or
callback ABI, even though it ships inside the Vutnet VPM archive.

## 7. Vutnet integration boundary

The following responsibilities belong to the Vutnet native adapter, not the
internal server engine:

- build/validate the typed flattened Router list in Vut;
- read route data and retain each typed `DispatchPipeline._dispatch` callback
  through the public ABI;
- match Vutnet routes and build owned Context values;
- invoke the Vut-side middleware pipeline and Controller(Context), convert the
  final Response, and bind App.listen to Server;
- pass `App.with_max_body_bytes` into `ServerConfig`;
- translate process shutdown signals into the server `ShutdownHandle` and wait
  for the listener/connection drain before returning from the blocking call.

The Rust Handler boundary is an internal Cargo API. The Vutnet adapter reuses
it without adding a second router. The VPM archive has no local server-package
dependency or nested manifest. No dyn, raw pointer or private runtime handle is
used to shortcut the public callback ABI boundary.

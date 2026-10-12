# Vutnet — Native Rust Boundary

## 1. Responsibilities and dependencies

Rust owns network I/O, Hyper integration, route matching for the flattened
table, and conversion between server transport values and Vut public managed
values. Reuse mature crates rather than writing TCP, HTTP parsing, or an async
executor:

- Tokio for listener/runtime/synchronization;
- Hyper and hyper-util for HTTP server protocol and Tokio I/O adapters;
- http for methods, headers, status and message types;
- http-body-util and bytes for bounded, owned bodies;
- matchit for route matching after Vut has normalized and validated patterns;
- percent-encoding helpers for validated route/query decoding.
- cap-std for filesystem directory capabilities and symlink/reparse confinement;
- mime_guess for extension MIME lookup, mime for media-type validation;
- multer for bounded binary multipart parsing;
- tokio-util ReaderStream and futures-util for native streamed file bodies.
- html5ever 0.27 and markup5ever 0.12 for HTML5 serialization and escaping.
- anstream and anstyle for cross-platform terminal output, style definitions,
  TTY-aware ANSI handling and `NO_COLOR` support.

The internal `vutnet-server-native` Cargo crate owns the Tokio/Hyper transport
foundation. The Vutnet native adapter owns matching Vutnet paths and calling Vut
Controllers. Both crates are source components of the single `vutnet` VPM
package; Vut's public API exposes no Hyper/Tokio types, Rust Future, borrowed
Rust lifetime, or raw native callback address.

Vut's stdlib module `http` is an outgoing client. The internal Cargo crate is
named `vutnet-server-native`; it is not a VPM child package or a public `http`
namespace. A separate `net` package is not needed.

## 2. Native package boundaries

The low-level HTTP engine is an internal Cargo project at
`vpm-package/vutnet/native/server`:

~~~text
native/server/src/
├── lib.rs
├── server.rs       listener and serve lifecycle
├── connection.rs   Hyper connection service/body limit
├── config.rs       bind address and body limit
├── request.rs      owned ServerRequest
├── response.rs     owned ServerResponse → Hyper response
├── streaming.rs    move-only FileResponse → asynchronous Hyper body
├── handler.rs      server handler boundary
├── body.rs         bounded body collection
├── error.rs        server errors
├── lifecycle.rs    connection tracking
├── shutdown.rs     shutdown coordination
└── ffi.rs          native package entrypoints
~~~

The Vutnet adapter lives at vpm-package/vutnet/native:

~~~text
native/src/
├── lib.rs
├── identity.rs     canonical public ABI nominal type descriptors
├── ffi.rs          versioned typed App.listen entrypoints
├── lifecycle.rs    OS signals to graceful server shutdown
├── routing.rs      one-time route loading and request dispatch
├── request.rs      route matching and URI decoding
├── context.rs      owned Vut Context construction
├── response.rs     Vut Response to ServerResponse conversion
├── static_files/   capability roots, validated URL paths, native file replies
├── multipart/      multer parsing, owned snapshot and public ABI builders
├── terminal/       structured server events, aligned formatter and output
└── view/           public ABI reads, tree validation and HTML5ever serialization
~~~

The adapter depends on the public `vut-native-abi` and `vut-native-sdk` crates,
plus the internal server Cargo crate. It does not depend on private
compiler/runtime handle layouts. The Vutnet VPM package has no local VPM
subdependency; its native Cargo dependency points to `native/server` within the
same package.

## 2.1 Terminal output boundary

Server and routing code emit structured startup, request, request-error and
shutdown events through the private `terminal` module. Normal request lines
align method, path, status and duration. Internal failures use a separate
stderr block with a red bold heading, colored request/status, and aligned
Stage/Cause labels; the cause is sanitized for terminal control characters.
The formatter owns all color and layout so network code emits structured
fields only. `anstream` adapts stdout/stderr to the terminal and strips styling
for non-TTY output or `NO_COLOR`; `anstyle` defines the styles. Logs omit
headers, query values, bodies, peer addresses and request IDs. The current
native server still binds loopback and does not own source watching or rebuild
lifecycle. The Rust `vutnet` CLI owns those workflows. When the CLI starts an
app process it may set `VUTNET_CLI_MODE` and `VUTNET_CLI_WATCHING`; the existing
structured ready event displays those values without changing the public App
API or binding behavior.

`VUTNET_CLI_ROUTE_DUMP=1` is a private child-process marker used only by
`vutnet routes`. The normal typed native import still receives the flattened
`list[RouteEntry]`; after existing route validation the adapter serializes its
path/method projection to stdout and returns before constructing a Tokio
runtime or binding a socket. No C ABI symbol or runtime layout is exposed.
Startup route-table failures use the same colored formatter and preserve the failing entry/field or invalid method, duplicate method/path, and conflicting-pattern cause. This distinguishes malformed ABI data from invalid route configuration.

For `vutnet dev`, the supervisor passes the parsed `--verbose` choice to its
server child through a private environment marker. The handler tags a completed
response as application or static only from the actual selected handler; the
terminal logger suppresses successful static responses unless verbose mode is
enabled. Static errors and application responses remain visible. No path or
extension filtering is used.

## 3. Startup route-list boundary

The Vut side retains the typed public shape:

~~~vut
type Controller = fn(Context) -> Response
type RouterFactory = fn() -> Router

extern "C" fn _native_listen_v3(
  port: i32,
  max_body_bytes: i32,
  entries: list[RouteEntry],
  mounts: list[StaticMount]
) -> i32
~~~

The `vutnet_app_listen_v1(port, entries)` symbol keeps the 1 MiB default;
`vutnet_app_listen_v2(port, max_body_bytes, entries)` keeps its existing contract.
New callers use v3 with static mounts. All symbols keep their original argument
signatures. Package/native schemas are built together; this is not a promise to
load historical compiled Context layouts against an unrelated native artifact.

The source-level native import remains typed. The compiler chooses callback-ABI
marshalling when an import parameter's resolved type graph contains a callable.
RouteEntry contains controller: Controller, so list[RouteEntry] uses the
public callback ABI value transport. The compiler passes an opaque public
ValueHandle token; native code validates canonical type identity and reads
the list/nominal fields using public SDK wrappers. A plain managed import whose
type graph has no callable still follows direct C ABI checks and can be rejected
with E8004; Vutnet's route-table import does not use that shape.

Nominal ABI versions are compiler metadata (`@abi_schema(N)` on a Vut `data` or
`enum`, defaulting to `1`). The Vutnet `Response` schema is version `2`; its Rust
`NominalType` binding and generated `Controller(Context) -> Response` signature
derive that version from the compiler-generated VPM schema manifest. Other
Vutnet nominal schemas remain version `1` unless explicitly annotated.

The package also builds its `[[bin]]` CLI, whose Vut source uses only primitive
managed imports and does not make the HTTP nominal schemas reachable. In that
CLI-only build, the compiler manifest may omit those nominal IDs. The native
build support then uses source-declared schema defaults (including
`Response = 2`) solely for omitted types; every schema version present in the
manifest takes precedence. Framework applications that call the HTTP native
imports therefore keep using the exact compiler-emitted versions. If a Vutnet
nominal's `@abi_schema` changes, update its native build fallback alongside
that source declaration.

At startup the adapter:

1. reads the flattened list length and each entry;
2. reads canonical path, method names and the typed DispatchPipeline field;
3. promotes and retains each named/non-capturing `_dispatch` callable as a typed
   `Context -> Response` callback handle;
4. builds its native path/method matcher.

The RouterFactory and route list do not cross again per request. The compiler
and SDK own temporary value tokens according to the public ABI retain/release
contract. Native handles do not contain pointers into Vut object layout.

## 4. Request and Context ownership

The request pipeline is:

~~~text
Hyper parses request
    ↓
server collects a body up to its finite configured limit
    ↓
owned ServerRequest(method, path, query, headers, body, peer, request_id)
    ↓
Vutnet matches path/method and decodes route/query data
    ↓
public ABI builds owned KeyValue/list/Context values
    ↓
typed Callback<DispatchPipeline._dispatch>.submit((Context,))
~~~

No reference to Hyper request storage escapes its handler. The default body
limit is 1 MiB; `App.with_max_body_bytes` passes a validated nonnegative value
into `ServerConfig::with_max_body_bytes`. Malformed URI/query/route escapes and header values that
cannot be represented as UTF-8 return 400 before Controller invocation.
Unmatched paths return 404; a matched path with the wrong method returns 405
and an Allow header.

Context construction owns method/path/header/query/route-param strings, body
bytes, client IP/port and request ID. The public SDK builds these with typed
value operations and the Vut accessors read them as normal managed Vut values.

For matched routes, multer parses the bounded body once into owned form data.
The adapter builds nominal Multipart/MultipartField/MultipartFile values with
typed list/str/bytes/bool/int SDK operations. Private metadata presence flags
let Vut accessors return `str?` without transporting ABI optionals. Parsing
errors become snapshot error tags, interpreted as typed MultipartError in Vut;
partial success is discarded. No compiler/runtime/ABI extension is needed.

The public middleware dispatcher stays on the Vut side. Its `DispatchPipeline`
contains the typed Controller, global middleware list and inherited scope
layers. Rust retains and invokes the pipeline's ordinary typed
`fn(Context) -> Response` entrypoint through the same public callback ABI used
for Controller callbacks; it does not read individual middleware fields or add
a middleware-specific native symbol. The Context carries the continuation
cursor and a managed reference to that immutable pipeline while Vut executes
the onion chain.

## 5. Response ownership and conversion

The Vut Controller returns an owned nominal Response through the callback
operation result. The adapter validates and reads status/body/content type
through public ABI accessors, creates an owned ServerResponse, then releases
the temporary Vut response and callback operation owners exactly once. The
server consumes ServerResponse into Hyper's response once.

Current Vut constructors are `response.send(int, str) -> Response`,
`response.send_bytes(int, bytes) -> Response`,
`response.html(int, str) -> Response`,
`response.json(int, json.Value) -> Response` and
`response.json_encodable(int, json.Encodable) -> Response`. JSON uses stdlib
`stringify(Value)` and sets `application/json; charset=utf-8`; `Encodable` is an
explicit opt-in. `Response.with_header` validates token names and field values
in Vut and returns typed `HeaderError`; Rust validates the data again through
`http::HeaderName`/`HeaderValue`. Matching names are replaced case-insensitively.
For View pages, the overloaded `response.render(fn() -> View) -> Response` and
`response.render[T](fn(T) -> View, T) -> Response` call the shared View renderer
and map render/layout errors to HTTP 500; these remain Vut-level constructors
and do not add a second renderer at the native boundary. A failed View
render stores its safe reason in private `Response._diagnostic`; native logging
consumes this field and never includes it in the HTTP response. Ordinary
constructors leave it empty. Current compiler codegen emits nominal and callable
schema version 1, so Vutnet must use v1 for both Response and the Controller
return signature. `_diagnostic` is private development metadata; the Vutnet source
and native package must be rebuilt together. Public schema-version bumps remain
blocked until Vut supports source/compiler schema version selection. The generic
Native Callback ABI protocol remains v1.

No zero-copy borrow crosses the FFI boundary. ABI string/byte views are valid
only while an owning SDK value remains live; data copied into the server
transport owns its allocation independently. Error/early-return paths release
local handles through SDK ownership guards.

## 5.1 View SSR boundary

The `vutnet.view` managed import receives a canonical nominal View token, reads
the typed node list and each node's typed attribute list through
`vut-native-sdk`, validates parent indices and the HTML security policy, and
returns `result[str, str]`. The package does not inspect Vut object layouts or
use private Runtime Handles. HTML5ever owns context-aware escaping and void-tag
serialization; Vutnet owns its supported tag/attribute/URL policy. Rust ABI
read/schema failures return managed-import status errors; invalid View content
returns the declared Vut result error.

`crates/vpm/tests/vutnet_view_http_e2e.rs` proves a real VPM consumer can render
a View with resolved slots, escape text and attributes, reject unsafe URLs, set
`text/html; charset=utf-8`, and deliver the output to an HTTP client.

Static files bypass Context/Controller entirely after normal route precedence.
Retained cap-std directory capabilities open validated request suffixes;
opened files become move-only native FileResponse values. `server` consumes
them into ReaderStream bodies with bounded chunks, releasing file ownership on
completion/disconnect/drop. Existing buffered ServerResponse/Handler.handle
remain available; additive Handler.handle_response selects Buffered/File.
No public Vut streaming Response is claimed. Security/MIME/middleware contracts
are specified in `09-static-and-multipart.md`.

## 6. Callback execution, errors and lifecycle

Hyper may accept multiple connections and submit request work concurrently.
Native Callback ABI v1 serializes Vut callback execution on its runtime-created
worker. The blocking App.listen entry parks ordinary Vut execution while
requests are served, which fits the current root concurrency contract. This
does not make arbitrary Vut globals, captured resources, or Vut code generally
thread-safe.

`App.listen` maps Ctrl+C, Unix SIGTERM and Windows Ctrl+Break into the server's
`ShutdownHandle`. The accept loop stops and active connections drain before the
blocking call returns. Vut callers see no Hyper/Tokio types. The API is
signal-driven because the current Vut application model exposes a synchronous
blocking listener rather than a background server handle.

The ABI supports named/non-capturing callable fields. Capturing callbacks are
rejected because v1 does not transfer an environment with safe ownership.
Rust panic/unwind does not cross C ABI; ABI errors and failed callback
operations are mapped to deterministic HTTP 500 responses and logged with a
request stage plus the ABI status. Invalid Response fields are logged with the
field/conversion reason while the client still receives a generic 500. An
unhandled Vut trap still reaches the runtime abort path, writes its panic text
to stderr, and can abort the process; recoverable trap-to-HTTP conversion is
not available in this phase.

The ABI has no foreign-thread attach/detach or host-thread pump API. Native
packages must submit through the public callback registry rather than call a
generated Vut function pointer.

## 7. VPM native build and release boundary

`vutnet` is the only VPM package. Its root `native/build.toml` invokes
`native/Cargo.toml`; that Cargo crate depends on the internal
`native/server/Cargo.toml` crate, so VPM builds and links one native artifact
for the complete framework. There is no `[dependencies] server = { path = ... }`
and no nested `vpm.toml` for the server engine. The real-socket E2E copies one
Vutnet package into a temporary consumer and verifies the integrated native
build/link flow.

VPM source publication rejects local VPM dependencies and nested VPM projects.
Standalone runnable examples therefore live beside the distributable package
under `vpm-package/vutnet-examples/`, not inside its file tree. The native Cargo
manifest still refers to the Vut workspace crates `vut-native-abi` and
`vut-native-sdk` by repository-relative paths, and declares `vut-runtime` as a
repository-relative dev dependency. The trusted artifact build must provide the
workspace crates or switch them to versioned Rust dependencies before it can
build from an isolated registry source archive.

## 8. Verification and current status

The integration proof is crates/vpm/tests/vutnet_http_e2e.rs. It launches a
compiled Vut app and exercises real HTTP over loopback TCP, including text and
bytes responses, nested routes, parameters, query/header/body Context values,
metadata, global and scoped middleware, onion unwind, short-circuit,
late-registration and sibling isolation, 404/405, repeated requests and multiple
client connections.

ABI prerequisites are covered by crates/vut-compiler/tests/ffi_e2e.rs and
vut-types checker tests. The internal server crate's low-level HTTP, body and
shutdown behavior is tested separately in `native/server/tests`.

Phase 3 and Phase 4 API/lifecycle hardening are complete. The real HTTP E2E
covers JSON Value/model responses, response-header validation/replacement,
configured body limit and 413, malformed route escapes, invalid UTF-8 body,
invalid status, 256 repeated requests, independent connections and graceful
signal shutdown. Remaining limits are serialized callback entry,
non-capturing callbacks only, and abort-on-unhandled-trap behavior.

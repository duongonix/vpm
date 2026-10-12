# Vutnet — App, Context and Response

## 1. App lifecycle

The Vut application configures its router in Vut, then starts the blocking
native HTTP server from `App.listen`:

~~~vut
import vutnet
import vutnet.router as router

fn main() -> int:
  app = vutnet.app()
  app.router(router.RootRouter)
  app.listen(3000)
  0
~~~

`App.router` accepts a `RouterFactory` (`fn() -> Router`). `App.listen(port:
i32) -> i32` invokes that factory and `Router.flatten()` once, then passes the
typed route list to the Vutnet native adapter. `listen` blocks while the server
runs. Ctrl+C requests graceful shutdown; Unix also handles SIGTERM and Windows
handles Ctrl+Break. The listener stops accepting new connections and drains
active connections before `listen` returns `0`; negative codes report setup or
server failures. The synchronous Vut API has no separately callable stop handle.

`App.with_max_body_bytes(limit: i32) -> result[App, AppConfigError]` returns a
configured copy. The default is 1 MiB; zero accepts empty bodies only; negative
limits return `AppConfigError.invalid_body_limit`. `listen` passes the chosen
limit to the server before binding.

After a successful bind, the native adapter prints a compact Vutnet startup
header with the package version, startup duration and the actual loopback URL
(including the OS-selected port when `0` was requested). Successful and
application-selected HTTP responses log method, path, status and handler
duration. Internal request failures print a separate colored error block to
stderr with method, path, status, duration, failure stage and safe diagnostic;
the HTTP client still receives a generic 500 body. Request bodies, headers and
query values are never included. Normal lifecycle and request lines go to
stdout. Styling is adaptive: ANSI styling is removed for redirected output and
when `NO_COLOR` is set. The current API binds loopback; `App.listen(port)` is
the source of truth for the port. The official `vutnet dev` command is a
persistent supervisor: it builds a candidate before replacing the server child,
keeps the last successful server on compile failure, and waits for a later save
after a runtime crash. It does not rewrite App configuration or expose a
background server handle. CLI-provided mode and watching labels are display
context only. Compiler diagnostics are emitted by the compiler/VPM build API
and are not reformatted by the running server.
Startup failures before bind use the colored native `ERROR` logger. Route loading identifies the failing route entry/field or reports an invalid method, duplicate method/path, or conflicting pattern instead of one generic message.

`App.static_files(prefix: str, directory: str) -> void` registers static mounts.
Trusted roots are resolved from process cwd and opened before binding; invalid
prefix/root/duplicate configuration returns `-7` from `listen`. Router path
matches take priority; otherwise the longest boundary-matching mount wins.
GET/HEAD serve files through native streaming, bypassing Vut middleware. See
`09-static-and-multipart.md` for confinement, MIME and directory/error rules.

`App.use(middleware: Middleware) -> void` registers global middleware in call
order. It wraps every matched route, runs outside Router/Routes middleware, and
can short-circuit before the route pipeline. The typed `Middleware` signature
and scope behavior are specified in `04-middleware.md`.

When signal-driven shutdown drains active connections, `listen` prints a
single `Server stopped` lifecycle line before returning. Application/router
responses print an aligned request summary, including 404/405 responses. In
`vutnet dev`, successful static-handler responses (2xx/3xx) are suppressed by
default; static-handler errors remain visible, and `vutnet dev --verbose`
prints every request. Static classification comes from the actual
`StaticFiles::response` handler selection after Router precedence, not from
the URL extension, so an application route such as `/api/theme.css` remains
visible. Internal failures use the richer error block instead. Handler
duration starts when Vutnet receives the owned `ServerRequest`; it does not
include socket accept or request-body collection.

## 2. Request/Controller/Response flow

~~~text
real HTTP request
    ↓ Hyper parses and server bounds/owns request body
owned ServerRequest
    ↓ Vutnet matches flattened route and builds an owned Context
global → Router/Routes middleware → Controller(Context)
    ↓ Response unwinds through middleware
final Response
    ↓ Vutnet consumes Response and creates ServerResponse
Hyper response
    ↓
real HTTP client
~~~

There is no public `Request` argument and no mutable response argument.
Controllers receive only `Context` and return the completed `Response`. The
native adapter uses the public Native Callback ABI and SDK; no Hyper request
pointer, private Runtime Handle ABI, or private Vut layout is exposed.

## 3. Controller type

The public controller callable is declared in `vutnet.router`:

~~~vut
type Controller = fn(Context) -> Response
~~~

This is a transparent function-type alias, while each Router/Route parameter is
statically checked against that exact signature. A zero-argument function or a
function with the wrong result type is rejected during compilation. Native
route loading preserves the callable signature identity and retains each
callback handle after reading the flattened route list at startup.

## 4. Context snapshot and ownership

`Context` is an owned Vut data value with private fields. Native construction
copies request-derived data through public ABI value builders; no field borrows
from Hyper or from a Rust stack frame. The Context includes:

- method and path strings;
- route parameters, decoded to owned UTF-8 strings;
- query pairs, decoded using form-style rules (`+` is a space);
- lower-case header names and UTF-8 values;
- owned request body bytes;
- owned bounded multipart snapshot or typed parsing error;
- direct peer IP and port, plus a request ID unique within one server run.

It is safe for a controller to return while Context values are being released;
the ABI's managed handles own their values independently for the duration of
the call. Request bodies default to the internal server engine's 1 MiB limit. An
oversized body is rejected with 413 before controller dispatch; callers can
choose another nonnegative limit with `App.with_max_body_bytes`.

## 5. Current Context accessors

These are the accessors implemented in `src/context.vut` today:

~~~vut
fn Context.method() -> str
fn Context.path() -> str

fn Context.param(name: str) -> str?
fn Context.param_or(name: str, fallback: str) -> str

fn Context.query(name: str) -> str?
fn Context.query_or(name: str, fallback: str) -> str

fn Context.header(name: str) -> str?
fn Context.header_or(name: str, fallback: str) -> str

fn Context.body_bytes() -> bytes
fn Context.body() -> result[str, Utf8Error]
fn Context.multipart() -> result[Multipart, MultipartError]

fn Context.client_ip() -> str
fn Context.client_port() -> int
fn Context.request_id() -> int
~~~

Absence is represented by `T?`; the `_or` variants return their fallback.
`method()` currently returns the HTTP method text as `str`, not a typed
`Method`. `path()` contains the encoded URI path without query. `param` is
case-sensitive and returns the first matching route parameter. `query` returns
the first matching decoded pair; repeated query keys remain stored in Context
but there is no `query_all` API yet. Header names are matched case-insensitively
by lower-casing the requested name; `header` returns the first matching value.
Invalid UTF-8 header values and invalid percent-encoding in route/query data
cause a 400 response before controller invocation.

`body_bytes()` returns the owned body bytes. `body()` performs strict UTF-8
conversion and returns `result[str, Utf8Error]`; invalid UTF-8 is not replaced.
An empty body is an empty `bytes`/`str`, never null.

`Multipart`/`MultipartFile`/`MultipartError` are imported from `vutnet.multipart`.
`multipart()` reads the parsed snapshot without consuming the raw body or
reparsing. `field`/`file` return the first optional value; `fields`/`files` retain
repeated values in wire order. Files expose name, optional filename/content type,
bytes and size. Fields use strict UTF-8; uploads remain bounded binary bytes in
memory and never become filesystem paths. Parse errors are typed values for the
application to map to 400; oversized requests get native 413 before dispatch.
Full signatures and error policy are in `09-static-and-multipart.md`.

`client_ip()` and `client_port()` describe the direct TCP peer. Forwarding
headers are not trusted. `request_id()` is only for request correlation, not a
security token. `host`, all-values header access, repeated-query access, and
TLS/proxy metadata are not in the current API.

## 6. Response value and constructors

`Response` is a public nominal value with private fields. It is constructed by
functions in `vutnet.response`, returned from the controller and consumed once
by the native adapter:

~~~vut
data Header:
  _name: str
  _value: str

enum HeaderError:
  invalid_name
  invalid_value

data Response:
  _status: int
  _body: bytes
  _content_type: str
  _headers: list[Header]
  _diagnostic: str # private, empty unless Vutnet generated an internal error
~~~

Current constructors:

~~~vut
fn send(code: int, content: str) -> Response
fn send_bytes(code: int, content: bytes) -> Response
fn html(code: int, content: str) -> Response
fn json(code: int, content: json.Value) -> Response
fn json_encodable(code: int, content: json.Encodable) -> Response
fn Response.with_header(name: str, value: str) -> result[Response, HeaderError]
~~~

`send` stores the UTF-8 body and sets `Content-Type: text/plain; charset=utf-8`.
`send_bytes` preserves the bytes and sets `Content-Type:
application/octet-stream`. Vut does not implicitly convert `bytes` to `str`.
`html` stores owned UTF-8 and sets `Content-Type: text/html; charset=utf-8`;
application code should pass HTML produced by `vutnet.view.render` or another
trusted serializer.
The adapter validates the status against HTTP status codes while building the
native response; an invalid response is converted to a deterministic 500.
`json` serializes an explicit `json.Value` with `json.stringify`;
`json_encodable` accepts types implementing `json.Encodable`. The compiler
synthesizes this method for eligible non-generic `data` records with supported
JSON fields. Other types may provide a custom method; unsupported shapes are not
silently reflected or stringified. Both set `Content-Type: application/json; charset=utf-8`.

`Response.with_header` validates HTTP token names and field values and returns
`HeaderError.invalid_name` or `HeaderError.invalid_value` on failure. It returns
a new Response; an existing name is replaced case-insensitively, including a
constructor's Content-Type. Repeated calls therefore keep one value per name;
there is no append/multi-value API. The Rust adapter validates again through
the `http` crate and maps malformed ABI data to a deterministic 500.

~~~vut
import vutnet.context at Context
import vutnet.response at Response
import vutnet.response as response

fn HomeController(ctx: Context) -> Response:
  response.send(200, "Hello from Vut")

fn BinaryController(ctx: Context) -> Response:
  content: list[u8] = @[0, 255, 65]
  response.send_bytes(200, bytes.from_list(content))
~~~

There is no mutable `res.status`, `res.header` or `res.text` facade.

## 7. JSON response status

`vutnet.response` exposes `json(code, json.Value)` and
`json_encodable(code, json.Encodable)`. Current stdlib `json.encode` returns a
string and `Encodable.to_json() -> Value` is a total conversion. Parsing and
decoding errors remain `json.Error` and do not apply to response encoding.
The compiler may synthesize `Encodable` for eligible record data; neither constructor uses runtime reflection.

## 8. Native HTTP behavior covered by E2E

The real-socket test `crates/vpm/tests/vutnet_http_e2e.rs` exercises:

- GET `/` and text `Response`;
- nested mounted routes and `/users/:id` route parameters;
- query, method, header and UTF-8 request body access through Context;
- peer IP/port and request ID;
- binary `Response` bytes;
- JSON `Value` and explicit `Encodable` responses with JSON Content-Type;
- case-insensitive response-header replacement and typed validation errors;
- global/scoped middleware onion order, response modification and short-circuit;
- late middleware registration and sibling-scope isolation;
- a configured request-body limit with 413 rejection;
- invalid status mapped to 500;
- malformed route escapes and invalid UTF-8 body handling;
- 404 for an unmatched path and 405 for a method mismatch;
- 256 repeated requests, six separate client connections, and graceful process
  shutdown after listener/connection drain.

The separate `crates/vpm/tests/vutnet_view_http_e2e.rs` test builds a VPM
consumer and verifies a resolved Vut layout rendered through the public managed
ABI, escaped HTML, Content-Type, rejected unsafe URLs and actual HTTP delivery.

The request list crosses the public ABI once at startup. Rust retains typed
Controller handles and uses that retained route table for matching. Multiple
Hyper connections are supported; Vut callback execution remains serialized by
the current callback ABI contract. Shutdown is process-signal based because
`App.listen` is synchronous; a separate programmatic stop handle/background
server API is not exposed.

## 9. View rendering

`vutnet.view` owns the typed `View` tree, slot/layout resolution and SSR. The
public renderer returns an explicit result; `Response` remains the controller's
return type:

~~~vut
import vutnet.view at View, view
import vutnet.response as response

data DocsData:
  title: str

fn DocsPage(page_data: DocsData) -> View:
  view:
    h1:
      text(page_data.title)

fn PageController(ctx: Context) -> Response:
  response.render(DocsPage, DocsData(title = "Documentation"))

fn HomePage() -> View:
  view:
    h1:
      text("Home")

fn HomeController(ctx: Context) -> Response:
  response.render(HomePage)
~~~

`response.render(fn() -> View) -> Response` and
`response.render[T](fn(T) -> View, T) -> Response` are overloads of the same
public function. Each invokes a statically typed Page, then uses the existing
View layout/slot composition and HTML5ever-backed renderer. A Page/data type
mismatch is a compile error. Render/layout failures become HTTP 500 with a
generic client body; the detailed render reason is written only to the native
request-error log. The private `_diagnostic` field is transport metadata, not
an HTTP header/body field. Layouts are ordinary typed functions returning `View`; `view(layout = SiteLayout):`
composes the result, and typed layout arguments can be supplied by calling a
layout factory in that expression. `slot(name = "body")` declares a required
slot, while a slot receiver body declares optional fallback content. The
low-level `views.render(View) -> result[str, str]` remains available, and
`response.html` remains the constructor for already-rendered HTML. There is no
`vut.render` API or second template engine. See `10-view-system.md`.

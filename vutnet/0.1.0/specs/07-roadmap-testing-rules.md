# Vutnet — Roadmap and Testing Rules

## 1. Source of truth and phase rule

Vutnet specs are design references. For Vut syntax, types, ABI, runtime
concurrency, ownership and VPM behavior, follow the root specs and the current
implementation. Do not replace a failing boundary with `dyn`, reflection, a
raw pointer, a private Runtime Handle ABI, or a Vutnet-specific ABI shortcut.

Each phase must use the real boundary it claims to validate. In particular, the
HTTP integration phase requires a real socket, Hyper, the actual flattened
Router and an actual Vut Controller; a mock handler or fixed response is not
proof of that path.

Native server terminal output is covered by focused formatter checks for
startup layout, request-column alignment, method/status styles, duration
formatting, control-character escaping, plain output and shutdown/error forms.
Per-request internal failures use a separate colored stderr block with stage and
cause; the logger remains a structured adapter, and network code must not add
ANSI sequences.

## 2. Phase status

~~~text
Phase 0    survey and spec reconciliation                     DONE
Phase 1    general Native Callback ABI + managed value bridge  DONE (v1 slice)
Phase 2    internal server engine with Hyper/Tokio              DONE
Phase 3.2  Vut Router DSL, composition and validation           DONE
Phase 3    Vutnet HTTP request → Controller → Response E2E      DONE
Phase 4    public API completeness and lifecycle hardening      DONE
Phase 5    typed middleware pipeline and HTTP integration        DONE (socket E2E)
Phase 6    static files + typed multipart                        DONE (socket E2E)
View 0–4   typed tree, slots, SSR, typed click ownership           DONE
View 5     browser/DOM typed click dispatch                         DONE (Chromium E2E)
View 6     mounted View state subscriptions and reactive refresh     DONE (typed bindings + eager compatibility)
View 7     SSR example and release evidence                        DONE (SSR surface)
View 8.1   typed SSR-to-WASM bootstrap contract                      DONE
View 8.2   shared call-site/occurrence hydration plan                 DONE
View 8.3   browser hydration with DOM reuse                           DONE
View 8.4   dynamic tree, mismatch and cleanup correctness             DONE (incremental DOM)
View 8.5   dev/run/build/start browser assets and hydration             DONE (real Chromium E2E)
View 8.6   client navigation, forms, prefetch + History API             DONE (real Chromium E2E)
View 9     retained controls/components/ID-keyed list DOM patching      DONE (real Chromium E2E)
Release    isolated native build + publish qualification       BLOCKED
~~~

The View 8 design and implementation status are recorded in
`specs/13-ssr-hydration.md`. The browser validates the typed Page, shared
site/occurrence plan and existing DOM before attaching listeners/subscriptions;
initial hydration preserves DOM identity. State writes reevaluate the View and
patch text/attributes/order on retained nodes. IDs anchor reorderable items.

View 8.5 connects that existing browser runtime to `vutnet dev`: VPM derives
a typed Page registry from checked `render_hydratable` call-sites, builds the
WASM client and generated JS bindings for each candidate, and Vutnet serves them
from its generation directory. `crates/vpm/tests/vutnet_dev_hydration_e2e.rs`
exercises the real supervisor, Hyper server, VPM client build and Chromium
click/state/DOM flow without a consumer client bin or browser config. Normal
`response.render` stays SSR-only. Dev/run/build share automatic browser staging;
production start serves the packaged manifest without a compiler.

The Phase 3 acceptance path now passes:

~~~text
real HTTP client
  → real TCP socket
  → VPM-built Vutnet package (adapter + internal server Cargo crate)
  → Hyper request
  → retained flattened Router table
  → owned Context via public ABI
  → global/scoped middleware dispatcher
  → typed Controller(Context) -> Response
  → owned ServerResponse
  → Hyper
  → real HTTP client
~~~

The View Phase 3 acceptance path also passes through the real package and
socket boundary:

~~~text
Vut View + resolved slots
  → managed Native SDK import (public nominal/list ABI)
  → HTML5ever serializer
  → result[str, str]
  → response.render(Page, typed_data)
  → Hyper socket
  → real HTTP client
~~~

## 3. Phase 0 — Survey (DONE)

The survey compared Vutnet documents with Vut root language, FFI, concurrency,
runtime and VPM specs and the corresponding source/tests. Decisions carried
forward:

- controller signature: `fn(Context) -> Response`;
- router factory signature: `fn() -> Router`;
- `Use(middleware)` registers Router/Routes middleware;
- `UseRouter(path, router)` composes a RouterFactory;
- endpoint method defaults to GET;
- immutable Response values are built in `vutnet.response`;
- VPM ships one `vutnet` package; `vutnet-server-native` is an internal Cargo crate under `native/server`;
- Rust uses mature Tokio/Hyper/http/body/bytes crates rather than a custom
  network or HTTP stack.

Vut supports typed function aliases and statically checks the callable
parameters. Receiver function calls use `=` for named arguments and `:` for
the trailing receiver body, for example `router:`.

## 4. Phase 1 — Public Native Callback ABI (DONE, supported v1 slice)

The public ABI and Rust SDK provide versioned opaque runtime, callback, value,
operation and error handles; canonical nominal/structural type identity;
managed list operations; nominal data build/access; callable-field promotion;
and typed callback invocation. ABI E2E covers a nested route entry/list with
`Controller(Context) -> Response`, signature mismatch, repeated invocation,
and package-boundary type identity.

For source-level native imports, the compiler selects the callback-ABI import
path when a parameter's recursively resolved type graph contains a callable.
The source signature remains typed. Generated marshalling retains the Vut value
for the native call and passes a public opaque value token; the native SDK
validates the requested type and owns retained handles explicitly. This is
general compiler/ABI behavior used by `list[RouteEntry]`; it is not a
Vutnet-only symbol.

Current v1 limits:

- only named/non-capturing callables can be promoted into managed callback
  fields; capturing closure environments are rejected;
- callback entry is serialized through the runtime callback worker, not
  executed concurrently with normal Vut code;
- the ABI has no foreign-thread attach/detach or host-thread pump API;
- an unhandled Vut trap still follows the runtime abort path; it does not unwind
  across C ABI, but it is not recoverable as a callback result.

The general ABI contract lives in
`specs/ffi/07-native-callback-abi-proposal.md`; concurrency rules live in
`specs/20-concurrency.md`.

## 5. Phase 2 — internal server engine (DONE)

`native/server` is the `vutnet-server-native` Cargo crate inside the single
Vutnet VPM package. It owns Tokio, Hyper, listener/connection lifecycle, bounded
request bodies, and owned server request/response transport. Its independent Rust tests
cover real sockets and HTTP behavior. The default maximum request body is 1
MiB. `App.with_max_body_bytes` validates and passes a custom nonnegative limit
to the engine.

The server engine does not know about Vut Router DSL, Context or Controller;
those belong to the Vutnet adapter above it.

## 6. Phase 3.2 — Router DSL (DONE)

The pure-Vut DSL builds and validates one deterministic flattened list:

~~~vut
type Controller = fn(Context) -> Response
type RouterFactory = fn() -> Router

fn router(body: fn(RouterScope)() -> void) -> Router
fn RouterScope.Route(path: str, controller: Controller, method: list[Method] = @[Method.get]) -> void
fn RouterScope.Routes(path: str, body: fn(RoutesScope)() -> void) -> void
fn RouterScope.Use(middleware: Middleware) -> void
fn RouterScope.UseRouter(path: str, router: RouterFactory) -> void
fn RoutesScope.Route(path: str, controller: Controller, method: list[Method] = @[Method.get]) -> void
fn RoutesScope.Routes(path: str, body: fn(RoutesScope)() -> void) -> void
fn RoutesScope.Use(middleware: Middleware) -> void
fn RoutesScope.UseRouter(path: str, router: RouterFactory) -> void
fn App.use(middleware: Middleware) -> void
fn Router.entries() -> list[RouteEntry]
fn Router.flatten() -> result[list[RouteEntry], RouterError]
~~~

~~~vut
type Next = fn(Context) -> Response
type Middleware = fn(Context, Next) -> Response
~~~

Calls use `=` for named arguments. `Routes(path = "..."):` opens a nested
receiver scope. `UseRouter` evaluates its RouterFactory once and mounts that
router's entries. `Use(middleware)` registers middleware in the current lexical
scope. Empty path and `/` are root; non-root parts are joined with
one slash, one leading slash and no trailing slash. Invalid paths and duplicate
`(method, path)` entries fail validation; registration order is preserved.

Evidence: Vutnet Router source tests and
`crates/vpm/tests/vutnet_router_compile_fail.rs` cover routing, flattening,
invalid controller/factory signatures and import boundaries.

## 7. Phase 3 — HTTP integration (DONE)

### 7.1 Startup route transfer

Vut calls its RouterFactory and `Router.flatten()` before listening. The
`list[RouteEntry]` crosses into the Rust adapter once through the public typed
Native Callback ABI import path. Rust reads canonical path, method names and
the typed DispatchPipeline dispatcher, then retains its callback handle in the
route table. Requests do not call the factory or reread the Vut list.

This managed import path is selected because `RouteEntry.controller` has a
callable signature. The compiler preserves source types and emits public
value-handle marshalling. The public ABI list/nominal/callback APIs do the
transport; the adapter does not use a private runtime handle or raw-pointer
shortcut.

### 7.2 Per-request pipeline

For each request, Hyper/server creates an owned `ServerRequest`. The Vutnet
adapter matches method/path against the retained route table, builds an owned
`Context` through public ABI value builders, submits the retained
`Controller(Context)`, reads the returned typed `Response`, and moves its
status/body/content type into an owned `ServerResponse`. Hyper consumes that
response once.

No pointer into Hyper request storage escapes. Context body/strings, response
bytes and callback handles use public managed owners with explicit retain and
release. Callback calls remain serialized by ABI v1. The E2E checks repeat
requests and separate HTTP connections through the same process.

### 7.3 Real socket evidence

The required real client test is
`crates/vpm/tests/vutnet_http_e2e.rs`. It builds a temporary VPM consumer with
a local `vutnet` dependency, compiles its adapter and internal server Cargo
crate together, launches the Vut app, and uses real HTTP clients over loopback
TCP. It checks:

- GET `/` → HTTP 200 and `Hello from Vut`;
- nested mounted `/admin/setting`;
- `/users/:id` route param and query lookup;
- POST method, path, query, header and UTF-8 body in Context;
- peer IP/port and request ID metadata;
- byte response with exact `[0, 255, 65]` body;
- JSON `Value` and `Encodable` responses with the JSON content type;
- case-insensitive response-header replacement and invalid-header rejection;
- configured body limit (413), invalid status (500), malformed route escape
  (400), and invalid UTF-8 request-body handling;
- 404 unmatched path and 405 wrong method;
- 256 repeated requests and six distinct client connections;
- signal-driven graceful shutdown after the server drains active connections.

The same test also verifies middleware on real requests: two global callbacks,
parent and nested scope inheritance across `UseRouter`, before/after onion
ordering, response-header mutation, short-circuit without Controller execution,
late lexical `Use` registration, sibling isolation, repeated requests and
multiple connections.

The Phase 6 regression rerun passed the real HTTP/middleware test and all 11
router signature tests, including the positive type-check case. The previously
recorded E8004 stdlib import failure does not reproduce in the current checkout.

The E2E drives graceful shutdown through the blocking `App.listen` lifecycle:
Unix SIGTERM and Windows Ctrl+Break are sent to the child process, which must
exit successfully after draining. Ctrl+C is also handled by the application.
There is no separate programmatic stop handle or background-server API.

## 8. Phase 4 — API completeness and lifecycle hardening (DONE)

Phase 4 stayed within the existing Controller/Router/ABI architecture:

1. `response.json(code, json.Value)` uses total `json.stringify`; the typed
   `response.json_encodable(code, json.Encodable)` path calls `to_json() ->
   Value`. The compiler now synthesizes this method for eligible non-generic
   record data; custom methods remain available. Both set
   `application/json; charset=utf-8`. Encoding is total because `json.stringify`
   returns `str`, not `result`.
2. `Response.with_header(name, value)` returns typed `HeaderError`, validates
   field syntax in Vut, returns an immutable Response, and replaces matching
   names case-insensitively. Rust validates again with the `http` crate; no
   duplicate-value append API is exposed.
3. `App.with_max_body_bytes(limit)` returns `result[App, AppConfigError]`; the
   default remains 1 MiB and the chosen bound reaches `ServerConfig`. Blocking
   `App.listen` maps Ctrl+C, Unix SIGTERM and Windows Ctrl+Break to graceful
   accept-stop/connection-drain without exposing a Hyper/Tokio handle.
4. The real-socket VPM E2E covers JSON Value/model bodies, response headers,
   413 for the configured limit, invalid status→500, malformed route escape,
   invalid UTF-8 request body, repeated requests, multiple connections, and
   successful signal-driven shutdown. Context construction, callback submission/execution,
   and Response conversion failures map to generic HTTP 500 and emit a colored
   server-only request-error block with stage and safe cause. A source Vut Controller
   cannot produce an arbitrary callback ABI error with its statically fixed return
   type, and an unhandled Vut trap still aborts before request logging can recover it.
5. The E2E runs 256 repeated Controller invocations and exits only after the
   server drains. The current public SDK exposes no live managed-handle or
   allocation counter, so platform heap-growth assertions are unavailable;
   callback ownership/drop behavior remains covered by Native Callback ABI
   tests, and process shutdown cleans up remaining process-owned resources.

The `Controller(Context) -> Response` signature and one-time flattened
route-list transfer are unchanged. A separately callable programmatic stop
handle/background server API is still outside the synchronous Vut application
model and remains a future design item.

## 9. Phase 5 — Typed middleware (DONE)

The public API is `type Next = fn(Context) -> Response` and
`type Middleware = fn(Context, Next) -> Response`. `App.use` adds global
middleware in call order; receiver `Use(middleware)` adds middleware to the
current Router/Routes lexical scope. `UseRouter(path, router)` is the only router
mount API. These names are distinct and the previous `Use(path, router)` mount
form is removed.

Vut builds a typed `DispatchPipeline` after the Router receiver body returns.
Every `RouteEntry` carries the route's pipeline dispatcher. Rust retains that
typed `Context -> Response` callback through the general Native Callback ABI and
does not inspect or invoke middleware callbacks individually. Global middleware,
outer scopes, nested/mounted scopes and Controller execute as an onion; a
middleware may short-circuit or return an immutable modified Response. Middleware
registered later in the same lexical receiver scope still applies to earlier
routes, and scopes do not leak to siblings.

No compiler/runtime/ABI change was required. `Next` is the typed Vut dispatcher
stored in `DispatchPipeline`; the request Context carries the continuation
position and managed pipeline reference. ABI v1 serializes callbacks and only
supports named/non-capturing middleware functions. Multiple `next(ctx)` calls
run downstream more than once; V1 does not add a one-shot guard. Unhandled Vut
traps retain the runtime abort behavior.

The real-socket acceptance evidence is in `crates/vpm/tests/vutnet_http_e2e.rs`;
typed signature rejection is in `crates/vpm/tests/vutnet_router_compile_fail.rs`.

## 10. Later phases

### Phase 6 — Static files and multipart (DONE)

`App.static_files` registers safe native mounts; ordinary Router path matches
win, then the longest matching static prefix. Static responses bypass Vut
middleware and stream opened files with MIME/Content-Length; HEAD sends no body.
Capability-relative resolution confines symlink/Windows junction access.

`Context.multipart()` returns a typed result over an owned, once-parsed snapshot.
Multer parses bounded binary parts; first/all field/file accessors preserve
repeated values. Uploads remain in memory, original filenames remain metadata,
and malformed input is a typed error for application response policy. Existing
body bounds reject oversized Content-Length/chunked input with 413 before
dispatch. No compiler/runtime/VPM or public callback ABI changes are required.

Evidence: `vutnet_static_multipart_e2e` launches one VPM-built Vut app with
static mounts, Router, global/scoped middleware and upload controllers. It checks
13 MIME extensions, text/binary/empty/nested/8 MiB files, HEAD, 404/405,
Router precedence, longest mount, raw traversal/Windows path attacks and
outside-root junction rejection; multipart text/repeated/multiple/empty parts,
filename/content type, exact non-UTF-8 file bytes, malformed/boundary/disposition
errors, exact/over body limits, chunked 413, middleware short-circuit, 32 static
and 64 binary-upload repeats, six separate connections for each feature, and
graceful process exit. Native tests additionally exercise the 1024-part bound,
duplicate part-header normalization, invalid part MIME and startup configuration validation.
See `09-static-and-multipart.md` for contracts and remaining limitations.

### View phases 0–7 and hydration/navigation steps 8.1–8.6 DONE

`specs/10-view-system.md` documents the pure-Vut tree builder, deterministic
slot resolution, HTML5ever-backed SSR, validation/security policy,
overloaded `response.render(Page)` and `response.render(Page, page_data)`, plus
Phase 4 typed click event ownership. The View tests cover typed dispatch, wrong
signatures, target validation and captured Vut closure cleanup. The separate
HTTP E2E builds a temporary VPM consumer and verifies typed/no-data Pages,
layout/slots, data iteration, escaping, render failure to HTTP 500,
Content-Type, repeated requests and real HTTP delivery.

The opt-in SSR hydration path has a typed `response.render_hydratable` bootstrap
and shared call-site/occurrence plan. `hydrateView` validates the evaluated View
and DOM before attaching typed events and state subscriptions without replacing
the initial DOM. The real headless-browser test covers layout/slots, nested
components, loops, conditionals, callback dispatch, state-driven refresh,
mismatch detection, explicit replacement fallback, repeated interaction and
unmount cleanup. Typed text/attrs/children bindings evaluate only observed
callbacks and patch their nodes/regions; eager reads retain full-render compatibility.
Unique IDs support list moves.
Reactive E2E verifies drafts, focus/selection, checkbox/select, controlled values,
component/list reorder/insert/delete, coalescing and unmount cleanup.
The server-state pipeline compiles direct typed Page functions for both SSR
and browser WASM. VPM discovers each `render_hydratable` call-site from the
checked program and emits a registry entry plus typed factory wrapper; ordinary
page data uses compiler-generated JSON encoding, and a declared static
`from_json` is called by an automatically generated typed decoder. Vutnet builds
and stages the registry/client for dev, run and build, then selects the factory
from the bootstrap Page ID. No client bin, factory or decoder config is authored
by an application. The real-browser dev/run/start E2E covers this path.
View 8.6 adds same-origin GET navigation through the existing server Router,
`pushState`/`popstate`, hydration of fetched SSR documents and fallback to native
navigation for unsupported responses. Its delegated form support serializes GET
controls and supports URL-encoded/multipart POST with submitter values; 303
Post/Redirect/Get is the recommended POST response. Loading lifecycle events and
opt-in, cache-header-gated link prefetch are also supported. The real-browser
dev/run/start E2E covers links, GET and POST forms, prefetch reuse, Back/Forward
and SSR-only fallback.
The keyed-control browser regression additionally checks unkeyed descendants of
keyed component roots, repeated reorder, radio groups, indeterminate checkbox,
file selection, single/multiple select and option identity, textarea/nested
two-axis scroll, backward selection, deleting the focused item, reinsertion,
explicit declaration overrides, new checked radio priority, duplicate-key
rejection/recovery and removal of retained event listeners.

The hydration E2E compiles the same Page source to browser WASM and a native
VPM-built Vutnet/Hyper app, fetches its SSR HTML over a real socket, and hydrates
those exact bytes in Chromium. It does not synthesize SSR markers or DOM in
JavaScript. The dev hydration E2E additionally verifies automatic asset build,
serving and browser bootstrap, proves typed clicks update the DOM without a
document request, and exercises same-origin client navigation plus native fallback.

The general WASM binding runtime supports canonical nominal `data` schemas,
typed callback handles, and reusable/freeable temporary allocations.
Real headless Chromium E2E covers nominal ClickEvent marshalling, repeated click
dispatch, modifier metadata, in-flight listener removal, host disposal, and stable
memory pages. `api.listenClick` is a generic browser-host API and uses the
existing typed Vut callback signature; it does not add a Vutnet-only ABI.

View Phase 5 covers typed event dispatch and mounted DOM listener ownership.
View Phase 6 tracks cells read during synchronous mounted renders, schedules
affected typed binding evaluations per frame after `cell.set()`, with full-render
compatibility for eager reads, and
releases subscriptions/callbacks on refresh/unmount. Its Chromium E2E exercises
`vutnet.web.state`, captured callbacks, repeated updates, coalescing, stable
controls/component roots, ID-keyed list moves and cleanup. Fine-grained bindings
are DONE: execution-count assertions cover unaffected Pages/components, scoped
if/for/nested regions, switching dependencies, failed bindings and cleanup. SSR hydration reuses initial server HTML and is
tracked in View 8. The design and cell ownership sequence are in
root `specs/roadmap/reactive-state.md`. Phase 7 is complete for the implemented SSR
surface: `../../vutnet-examples/view-ssr/` is a standalone VPM app,
and `vutnet_view_http_e2e` checks rendered bytes and media type over real HTTP.
Template engines remain future work.
The SSR renderer is `vutnet.view.render(View) -> result[str, str]`; it does not
implement or reserve `vut.render`.

### Tailwind CSS integration

Tailwind integration is specified in `12-tailwind.md`. It adds optional,
strictly parsed `vutnet.toml`, exact tool-version resolution through VPM's
generic executable catalog, and a candidate transaction shared by application
and CSS builds. The persistent dev supervisor uses its existing watcher and
last-known-good replacement path; the one-shot Tailwind CLI does not write the
active CSS output during a candidate build. Implementation status is tracked
there and must not be marked complete until initial failure recovery,
subsequent CSS/application failures, candidate publication and last-known-good
rollback are covered by focused tests.
Release qualification should also smoke-test the pinned standalone CLI on each
supported host target; VPM's unit tests use a local HTTP fixture and do not
download the large executable.

Release qualification is currently blocked on an isolated native build: the
package has no local VPM dependency or nested VPM manifest, but
`native/Cargo.toml` still refers to `vut-native-abi`, `vut-native-sdk` and the
`vut-runtime` dev dependency by paths into the Vut monorepo. Before registry
release, either make those Rust crates available as versioned package
artifacts or provide a publisher-supported reproducible build context. Then run
publish-plan validation and native artifact target coverage. Do not publish
automatically.

### Official Vutnet CLI (implemented)

`vutnet new`, `dev`, `build`, `start`, `routes`, and `info` are dispatched by
`src/cli.vut`, the Vut source named by the package's `[[bin]]`. Rust
implementation lives in `native/src/cli/` and is reached through the existing
public managed native-import ABI in the same native package. `dev` is a
persistent supervisor with an OS-event watcher, debounced and coalesced
source/config changes, one interruptible `vpm build` transaction at a time,
and unique candidate artifacts under `.vut/dev/`. It compiles before stopping
the child server; compile failures preserve the running last-known-good
generation. Successful builds replace the managed process tree, runtime
crashes wait for a later save without an auto-restart loop, and Ctrl+C cancels
the build and reaps the server tree. `start` runs the existing release artifact
and never rebuilds. `routes` launches the built app in the private route-dump
mode and reads the actual flattened typed route list through the existing
native import. It does not parse source or bind a socket. The VPM
`vutnet_dev_e2e` real-process test launches the Vut CLI, verifies that a failed
compile preserves the live HTTP response, verifies that a successful rebuild
replaces it, and shuts down through Ctrl+C.
`RouteEntry` does not retain source Controller names, so output contains
canonical methods and paths only. The sibling
`../../vutnet-examples/cli-smoke/` VPM consumer checks the source dispatcher
through the native ABI. See `11-cli.md`.

## 11. Executable API example

~~~vut
import vutnet
import vutnet.router at router, Router
import vutnet.context at Context
import vutnet.response at Response
import vutnet.response as response

fn HomeController(ctx: Context) -> Response:
  response.send(200, "Hello from Vut")

fn RootRouter() -> Router:
  router:
    Route(path = "/", controller = HomeController)

fn main() -> int:
  app = vutnet.app()
  app.use(Logger)
  app.router(RootRouter)
  app.listen(3000)
  0
~~~

This follows Vut's required `fn main()` entrypoint form. Controller always
receives only Context and returns a Response.

## 12. Verification commands

Run focused tests while changing a phase; run the full appropriate workspace
gates after the feature is complete:

~~~text
cargo fmt --all -- --check
cargo check -p vut-runtime -p vut-stdlib-native -p vut-types -p vut-mir -p vut-codegen -p vut-compiler -p vpm
cargo test -p vut-compiler --test ffi_e2e transports_nested_typed_lists_and_controller_fields_across_public_abi
cargo test -p vpm --test vutnet_router_compile_fail
cargo test -p vpm --test vutnet_http_e2e
cargo test -p vpm --test vutnet_static_multipart_e2e
cargo test -p vpm --test vutnet_view_e2e
cargo test -p vpm --test vutnet_view_http_e2e
cargo test -p vpm --test vutnet_dev_hydration_e2e
cargo test --manifest-path vpm-package/vutnet/native/Cargo.toml --lib
cargo test --manifest-path vpm-package/vutnet/native/server/Cargo.toml
cargo fmt --manifest-path vpm-package/vutnet/native/Cargo.toml -- --check
cargo fmt --manifest-path vpm-package/vutnet/native/server/Cargo.toml -- --check
cargo clippy --manifest-path vpm-package/vutnet/native/Cargo.toml --all-targets -- -D warnings
cargo clippy --manifest-path vpm-package/vutnet/native/server/Cargo.toml --all-targets -- -D warnings
cargo clippy -p vut-runtime -p vut-types -p vut-mir -p vut-codegen -p vut-compiler -p vpm --all-targets -- -D warnings
~~~

The current phase is complete only when the real-socket test passes through
`app.listen()` → Hyper → flattened Router → typed Controller(Context) →
Response → Hyper → client.

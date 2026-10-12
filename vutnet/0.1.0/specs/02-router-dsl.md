# Vutnet — Router DSL

Static mounts are separate App configuration (`App.static_files`). A normal
Router path match wins over a static mount, including the Router's 405 result.
Otherwise the longest matching static prefix handles the request. Router DSL,
flattening and registration order are unchanged; see `09-static-and-multipart.md`.

## 1. Receiver Function syntax verified in Vut

The root Router builder is an ordinary lower-case function whose final
parameter is an inline Receiver Function type:

~~~vut
fn router(body: fn(RouterScope)() -> void) -> Router
~~~

The inline type is required for the trailing body to establish
`RouterScope` as the lexical receiver. A `RouterBody` type alias is accepted as
a function type, but a compiler spike showed that using that alias here does
not establish receiver-method lookup inside the body.

Use actual trailing-call syntax with parentheses:

~~~vut
fn RootRouter() -> Router:
  router:
    Route(path = "/", controller = HomeController)
~~~

`router:` is the canonical zero-argument call with a trailing receiver body;
the legacy `router():` spelling remains accepted for compatibility. The
lowercase name also avoids a symbol collision between a `Router` type and a
same-named constructor function; Vut declarations share one module symbol
namespace.

`Route` and `UseRouter` calls with named arguments use `=` just like other
function calls (`path = "/"`). `Use(middleware)` takes a positional callable.
A trailing receiver body is introduced by `:` after the call, as in `router:`.

Receiver methods are now visible across module and VPM local package
boundaries. `crates/vpm/tests/receiver_package_e2e.rs` proves an application
module calling `App.listen` and `Context.param` from a local package, and
`crates/vut-compiler/tests/receiver_module_e2e.rs` proves the same across
source modules. A receiver method that has an ordinary argument plus a trailing
receiver callback also works (the `Column(name, body)` shape in the package
regression test), so `RouterScope.Route`/`Use`/`UseRouter`, `App.router`/`use`/
`listen` and the `Context` accessors are usable from a consuming module. This is
verified current behavior, not a blocker.

Router and request integration module layout:

~~~text
src/
├── mod.vut              # package namespace
├── method.vut           # Method enum
├── context.vut          # owned Context + request accessors
├── response.vut         # immutable Response + send/send_bytes
└── router/
    ├── mod.vut          # public Router API and DSL
    └── path.vut         # internal path normalization
~~~

Vut has no re-export and no submodule aggregation, so the public Router symbols
are declared in `router/mod.vut`. Cross-module references use Vut relative
imports (`import ..method as m`, `import .path as paths`) so the package
resolves both when it is the current package and when it is mounted under its
package name. `router/path.vut` is an implementation detail.

## 2. Public callable types

Controller is one exact public callable signature:

~~~vut
type Controller = fn(Context) -> Response
~~~

RouterFactory is a normal zero-argument function type:

~~~vut
type RouterFactory = fn() -> Router
~~~

These aliases are transparent Vut aliases, but their function signatures are
still checked at each typed call site. A matching named function can be passed
through either alias. A no-argument function returning `int` is rejected when
a Controller is required; a compiler spike observed E1003. Do not accept an
arbitrary callable, `dyn`, or runtime-reflected signature.

The same spike invoked a `RouterFactory` value from a receiver method. Vutnet
evaluates each factory once while building the route configuration, not once
per request. A factory may use Vut closures because the factory itself runs in
Vut configuration code and is not the native request callback.

## 3. Desired Router tree and callable signatures

The desired typed DSL contains:

- `Router`: finished immutable route configuration;
- `RouterScope`: lexical receiver scope inside `router:`;
- `Route`: typed endpoint leaf;
- `Use`: typed middleware registration in the current Router/Routes scope;
- `UseRouter`: router-factory mount with a path prefix;
- `Controller`: `fn(Context) -> Response`;
- `RouterFactory`: `fn() -> Router`.

The builder signature keeps the Receiver Function type inline:

~~~vut
type Controller = fn(Context) -> Response
type RouterFactory = fn() -> Router
type Next = fn(Context) -> Response
type Middleware = fn(Context, Next) -> Response

fn router(body: fn(RouterScope)() -> void) -> Router

fn RouterScope.Route(
  path: str,
  controller: Controller,
  method: list[Method] = @[Method.get]
) -> void
fn RouterScope.Routes(path: str, body: fn(RoutesScope)() -> void) -> void
fn RouterScope.Use(middleware: Middleware) -> void
fn RouterScope.UseRouter(path: str, router: RouterFactory) -> void

fn RoutesScope.Route(
  path: str,
  controller: Controller,
  method: list[Method] = @[Method.get]
) -> void
fn RoutesScope.Routes(path: str, body: fn(RoutesScope)() -> void) -> void
fn RoutesScope.Use(middleware: Middleware) -> void
fn RoutesScope.UseRouter(path: str, router: RouterFactory) -> void
fn App.use(middleware: Middleware) -> void

fn Router.entries() -> list[RouteEntry]
fn Router.flatten() -> result[list[RouteEntry], RouterError]
~~~

The Router declarations were implemented in Phase 3.2; middleware aliases and
registration were added in Phase 5. `Method` is declared in `vutnet.method`;
`RouteEntry`/`RouterError`/`Router` in `vutnet.router`.
`App.router(router: RouterFactory) -> App` is also present. `App.listen` calls
the stored factory and `Router.flatten()` before the native boundary. A real
socket E2E now proves that the typed list crosses the public callback ABI once
at startup and the retained controllers serve requests (see
07-roadmap-testing-rules.md).

These signatures work in a single module and across module/package boundaries
with the current receiver-method support described above. The declarations show
parameter types and the GET default. In calls, receiver DSL functions use
`=` for named arguments:

~~~vut
Route(path = "/health", controller = HealthController)
UseRouter(path = "/admin", router = AdminRouter)
~~~

`Route` defaults to GET. Explicit multi-method endpoints use a typed list:

~~~vut
import vutnet.method as method

Route(
  path = "/health",
  controller = HealthController,
  method = @[method.Method.get, method.Method.head]
)
~~~

No default method is inferred from a controller. A `Routes` group uses the
signature in section 4.

## 4. `Routes(path = "/"):` groups

`Routes` is a receiver method on `RouterScope` with an ordinary `path`
argument and a trailing receiver callback:

~~~vut
fn RouterScope.Routes(
  path: str,
  body: fn(RoutesScope)() -> void
) -> void
~~~

The current compiler supports this shape. The general form — a receiver method
with an ordinary argument plus a trailing receiver callback — is proven by
`crates/vpm/tests/receiver_package_e2e.rs` (the `Column(name, body)` case) and
by `crates/vut-compiler/tests/receiver_module_e2e.rs`. `Routes` composes a
prefix with a nested scope:

~~~vut
fn RootRouter() -> Router:
  router:
    Routes(path = "/admin"):
      Route(path = "/setting", controller = SettingController)
      Route(path = "/", controller = AdminController)
~~~

V1 may use `Routes` groups and/or router factories with `UseRouter`. Both flatten to
the same canonical method/path entries. The group body establishes a
`RoutesScope` lexical receiver; nested `Routes` and leaf `Route` calls resolve
against it. Path normalization at each level follows section 6.

Composition via router factories remains valid and is not a fallback for a
compiler limitation:

~~~vut
fn RootRouter() -> Router:
  router:
    UseRouter(path = "/admin", router = AdminRouter)

fn AdminRouter() -> Router:
  router:
    Route(path = "/", controller = AdminController)
    UseRouter(path = "/setting", router = SettingRouter)

fn SettingRouter() -> Router:
  router:
    Route(path = "/", controller = SettingController)
~~~

Both forms compose to `/admin` and `/admin/setting`.

At server startup Vutnet consumes `Router.flatten()`'s returned
`list[RouteEntry]` once. The Rust adapter does not reimplement the Vut Router
DSL or call the RouterFactory per request; it retains the typed pipeline
dispatcher handles from those entries for subsequent matching/dispatch. This startup
transfer is covered by `crates/vpm/tests/vutnet_http_e2e.rs`.

## 5. `Use` registers middleware; `UseRouter` composes routers

`Use(middleware)` registers a typed middleware in the current Router or Routes
scope. It has no path argument and does not mount another router. Middleware
inheritance, late lexical registration, ordering and onion dispatch are specified
in `04-middleware.md`.

`UseRouter` evaluates a `RouterFactory` once, mounts its returned router below
the supplied prefix, and recursively composes its endpoints.

~~~vut
fn RootRouter() -> Router:
  router:
    UseRouter(path = "/admin", router = AdminRouter)
~~~

Both `Use` and `UseRouter` are available on `RouterScope` and `RoutesScope`.
`Routes(path = "/"):` groups are also supported (section 4).

## 6. Path normalization and composition

Route and mount paths are normalized before flattening:

1. An empty path and `/` both mean the root prefix.
2. A non-root path may include or omit a leading slash.
3. Strip leading/trailing slash characters and empty separators from each
   component before joining it to its parent.
4. Join non-empty components with exactly one slash, then validate the
   canonical path.
5. The canonical root remains `/`; other canonical paths have no trailing
   slash.
6. Reject query strings, fragments, control bytes, and invalid/empty named
   parameter segments.
7. A trailing slash on an incoming request matches the canonical route. A
   repeated internal slash in an incoming request is not collapsed into a
   different route.

Examples:

~~~text
join("/", "/")                  = "/"
join("/admin/", "/")            = "/admin"
join("/admin", "/setting/")     = "/admin/setting"
join("/api", "/admin/setting")  = "/api/admin/setting"
~~~

Nested `UseRouter` calls apply the same deterministic join at each level. They
cannot produce `/admin/`, `/admin//setting`, or duplicate prefixes.

V1 supports named path segments such as `/users/:id`. Invalid route patterns
fail configuration validation. The native matcher stores decoded parameter
values as owned UTF-8 data in Context. Reserved encoded path characters are
not silently rewritten in `Context.path()`.

Normalization is implemented in `router/path.vut`: `trim` strips edge slashes
and empty separators, `join` composes canonical paths, and `invalid_reason`
reports the first invalid canonical path (empty path, missing leading slash,
trailing slash, `//`, query/fragment, control byte, or empty/invalid `:name`
parameter). Since route components are normalized before validation, repeated
slashes inside a configured component collapse to one separator.

## 7. Router composition example

~~~vut
fn RootRouter() -> Router:
  router:
    UseRouter(path = "/api", router = ApiRouter)

fn ApiRouter() -> Router:
  router:
    UseRouter(path = "/admin", router = AdminRouter)

fn AdminRouter() -> Router:
  router:
    Route(path = "/setting", controller = SettingController)
~~~

Flattening yields:

~~~text
GET /api/admin/setting
~~~

Rust receives only immutable flattened route entries; it does not parse or
execute the Vut Receiver DSL. Each entry contains a canonical path, typed HTTP
method set, and a statically checked Controller association.

## 8. Route representation, validation and flattening

`Router` is a finished configuration with a private representation.
`Router.entries()` exposes the registration-order entries without validation.
`Router.flatten()` validates and returns `result[list[RouteEntry], RouterError]`.

The flattened endpoint representation and error type are:

~~~vut
data RouteEntry:
  path: str
  methods: list[Method]
  method_names: list[str]
  controller: Controller
  pipeline: DispatchPipeline

enum RouterErrorKind:
  duplicate_route
  invalid_path

data RouterError:
  kind: RouterErrorKind
  message: str
~~~

`DispatchPipeline` retains the typed Controller and middleware chain. Its
`_dispatch` callable has the same source signature as a Controller; Rust retains
and invokes that typed dispatcher and does not inspect middleware registrations.
See `04-middleware.md` and `05-native-rust.md`.

During configuration Vutnet must:

- invoke each `RouterFactory` exactly once (`UseRouter`);
- normalize mount and route paths (section 6);
- preserve endpoint method lists and the GET default;
- statically check every Controller as `fn(Context) -> Response`;
- reject empty/invalid parameter names and malformed route patterns;
- reject duplicate canonical `(method, path)` pairs;
- flatten `Router`/`UseRouter`/`Routes` into immutable entries in registration order;
- resolve each entry's inherited middleware pipeline after receiver callbacks finish;
- pass only the flattened representation to the native bridge.

`flatten()` rejects a path with a non-empty `invalid_reason` as `invalid_path`,
and rejects a repeated `(method, path)` key as `duplicate_route`. The same path
with different methods is allowed. The duplicate key is the canonical method
text plus the canonical path; comparison uses structural string equality, not
`list.contains`, which matches managed strings by identity.

`RouterBuilder` is an internal accumulation value shared by the lexical scopes.
It is declared without a `_` prefix only because Vut cannot construct a
`_`-prefixed type; its field is private and it is not a supported public type.

The route tree, factories and Receiver DSL do not run on the request hot path.

## 9. App registration

The application accepts a RouterFactory, not a pre-evaluated Router value:

~~~vut
app.router(router.RootRouter)
~~~

Its type is `fn() -> Router` through the `RouterFactory` alias. Passing
`RootRouter()` is not the public form.

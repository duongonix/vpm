# Vutnet — Middleware

Static mounts bypass the Vut middleware pipeline; they are native file responses.
Multipart route requests retain global/scoped middleware and onion semantics.
The parser stores success/error in Context before dispatch; malformed multipart
does not automatically short-circuit middleware. The controller/middleware maps
`MultipartError` to its Response. Body-limit rejection (413) occurs before this
pipeline. See `09-static-and-multipart.md` and the combined socket E2E.

## 1. Public typed API

Middleware uses the same immutable `Context` and `Response` values as a
controller. The public aliases are transparent Vut function types:

~~~vut
type Next = fn(Context) -> Response
type Middleware = fn(Context, Next) -> Response
~~~

`Next` and `Middleware` are available from `vutnet.context`; `vutnet` also
exports the aliases used by `App.use`.

Global middleware is registered on the application in registration order:

~~~vut
app = vutnet.app()
app.use(Logger)
app.use(Cors)
app.router(RootRouter)
app.listen(3000)
~~~

Router and `Routes` scopes use `Use(middleware)`. Router mounting has a
separate name and always takes a `RouterFactory`:

~~~vut
fn RootRouter() -> Router:
  router:
    Use(RootTrace)
    UseRouter(path = "/api", router = ApiRouter)

    Routes(path = "/admin"):
      Use(Auth)
      Route(path = "/", controller = AdminController)
~~~

The names have one meaning each:

~~~text
app.use(Middleware)                         global middleware
Use(Middleware)                             Router/Routes scope middleware
UseRouter(path = ..., router = RouterFactory) mount/composition only
~~~

There is no `Use(path = ..., router = ...)` overload and no `app.middleware`
alias.

## 2. Onion and short-circuit semantics

For a matching request, applicable middleware run from outer to inner. The
response then returns through them in reverse order:

~~~text
global A → global B → parent Router → mounted Router → Routes → Controller
        ←         ←              ←               ←        ←
~~~

A middleware can inspect Context, call `next(ctx)`, modify the returned
immutable Response, and return the modified value:

~~~vut
fn Logger(ctx: Context, next: Next) -> Response:
  out("before $(ctx.method()) $(ctx.path())")
  result = next(ctx)
  out("after $(ctx.path())")
  match result.with_header("x-processed", "yes"):
    ok(updated): updated
    err(_): response.send(500, "invalid middleware header")
~~~

Short-circuiting is a normal return and skips every downstream middleware and
the Controller:

~~~vut
fn Auth(ctx: Context, next: Next) -> Response:
  if unauthorized(ctx):
    return response.send(401, "Unauthorized")
  next(ctx)
~~~

Each call to `next(ctx)` resumes at the continuation position carried by that
Context. Middleware should call it at most once; ABI v1 does not add a mutable
one-shot guard, so deliberately calling it multiple times dispatches the
downstream chain multiple times.

## 3. Scope, inheritance and ordering

Middleware belongs to the lexical scope in which `Use` or `app.use` is
registered. Router scopes apply to their direct routes and descendants,
including mounted RouterFactory entries. `Routes` scopes apply to their
descendants, not sibling scopes. Middleware registration is resolved after the
Router receiver body has run, so a `Use` written after a `Route` still applies
to that Route.

The order is deterministic:

1. global middleware in `app.use` call order;
2. outer Router scope middleware in registration order;
3. nested Router/`Routes` scope middleware in registration order;
4. middleware in the mounted Router's own scopes;
5. Controller.

Composition through multiple `UseRouter` mounts applies each mount's parent
scope before the mounted Router's own middleware. Adding middleware to one
scope does not add it to sibling scopes. `UseRouter` invokes its factory while
the Router configuration is being built, once per mount; no Receiver DSL or
factory runs per request.

## 4. Flattened representation

`RouteEntry` retains its canonical path, methods, typed Controller and an
opaque Vut `DispatchPipeline` value. Vut builds pipeline layers after the
Router body returns:

- the application owns a flat list of global `MiddlewareEntry` values;
- each Router builder keeps flat scoped callback registrations plus integer
  scope-parent and registration-order tables;
- each route layer carries a managed registration list and the ordered scope
  IDs that apply to that route;
- a mounted route prefixes the parent's layer before the child's existing
  layers without extracting callable fields into a second callback list.

Rust receives the flattened `list[RouteEntry]` once at startup through the
public Native Callback ABI. It retains the typed `DispatchPipeline._dispatch`
function as the route callback. Rust does not inspect Middleware registrations
or schedule each middleware separately. On a request, that typed Vut
dispatcher reads the middleware tables, runs the onion chain and returns the
final `Response` to the existing server adapter.

The builder stores factories and callable registrations in flat typed lists
while Receiver callbacks are active. It creates nested `MiddlewareLayer`
values only after those callbacks return. This keeps Vut ownership explicit
and avoids copying managed callback values into duplicate per-route lists.

## 5. Callback, ownership and concurrency contract

`Next` is not a capturing closure. It is the statically typed `fn(Context) ->
Response` dispatcher already stored in `DispatchPipeline`; continuation
position and request data travel in the owned `Context` value. No `dyn`,
private Runtime Handle ABI, reflection or raw function pointer is exposed.

The request Context owns its method/path/parameters/query/headers/body and a
managed reference to the immutable pipeline. Context copies passed between
middleware retain their managed values. Middleware and Controller return
ordinary immutable Response values; each middleware may replace the value it
receives from `next`. The native adapter converts the final Response once to
`ServerResponse` and releases the ABI handles through the public SDK contract.

Middleware callbacks are invoked inside the Vut dispatcher on the existing
serialized Native Callback ABI worker. Hyper can accept multiple connections,
but Vut callback execution remains serialized in ABI v1. No foreign-thread
Vut entry is assumed or introduced.

Only named/non-capturing middleware callbacks are supported by ABI v1.
Capturing closures remain statically unsupported. A Vut trap still follows the
runtime abort path; middleware cannot catch it and turn it into an HTTP error.
These are callback ABI limits, not Vutnet-specific pointer workarounds.

## 6. Verification

The real-socket integration test is `crates/vpm/tests/vutnet_http_e2e.rs` and
uses the real VPM-built packages, Hyper listener, Router and Vut callbacks. It
covers global and scoped middleware, nested `Routes`, `UseRouter`, inherited
parent middleware, ordered before/after output, response-header modification,
short-circuit suppression of the Controller, late lexical registration,
sibling-scope isolation, repeated requests, independent connections and
graceful shutdown.

`crates/vpm/tests/vutnet_router_compile_fail.rs` checks that incorrect
Middleware arity/result/`Next` signatures and passing Middleware to
`UseRouter` are rejected statically. The route table, Controller and middleware
callbacks use typed public ABI transport; no Vutnet-specific native callback
entrypoint is added.

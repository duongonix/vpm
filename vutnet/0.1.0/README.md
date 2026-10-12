# vutnet

Native HTTP framework for Vut. It provides a typed Receiver Function router,
`Controller(Context) -> Response`, request context accessors, text/bytes/JSON
responses, validated response headers and a bounded Hyper/Tokio server.
It also serves confined static directories and parses typed multipart uploads.

## Build

```sh
vpm build
```

## Official CLI

The CLI executable is owned and versioned by this VPM package. After a
registry-ready Vutnet release is available, install it with:

```sh
vpm install vutnet
```

The installable command is declared in `vpm.toml` as a Vut `[[bin]]` and
dispatches from `src/cli.vut`; VPM builds and links it with Vutnet's native
package. Rust command implementation lives in `native/src/cli/` and is exposed
through the public managed native-import ABI.

```sh
vutnet new myapp
cd myapp
vutnet dev
```

`vutnet dev` stays running while it watches source/manifests. It builds a
candidate before replacing the server child; a failed build leaves the previous
successful server running, or waits for another save if no build has succeeded
yet. Ctrl+C shuts down the child and supervisor. For production use
`vutnet build` followed by `vutnet start`; `start` never builds implicitly.
`vutnet routes` lists routes from the app's real flattened Router, and
`vutnet info` reports the project/toolchain details available on this machine. See
[`specs/11-cli.md`](specs/11-cli.md).

## Start an application

```vut
import vutnet
import vutnet.router as router

fn main() -> int:
  app = vutnet.app()
  app.router(router.RootRouter)
  app.listen(3000)
  0
```

`App.with_max_body_bytes(limit)` returns a `result[App, AppConfigError]`; the
default is 1 MiB. `App.listen` blocks and drains active connections on Ctrl+C,
Unix SIGTERM or Windows Ctrl+Break.

Controllers return immutable values from `vutnet.response`: `send` for UTF-8
text, `send_bytes` for binary data, `json` for `json.Value`, and
`json_encodable` for explicit `json.Encodable` models. `Response.with_header`
returns a typed result and replaces matching header names case-insensitively.

## Middleware

Middleware receives the request `Context` and a typed continuation. Global
registration uses `app.use`; `Use` applies to the current Router/Routes scope;
`UseRouter` mounts a RouterFactory:

```vut
import vutnet
import vutnet.router as router
import vutnet.router at Router
import vutnet.context at Context, Next
import vutnet.response at Response
import vutnet.response as response

fn Trace(ctx: Context, next: Next) -> Response:
  value = next(ctx)
  match value.with_header("x-trace", "complete"):
    ok(updated): updated
    err(_): response.send(500, "invalid middleware header")

fn Home(ctx: Context) -> Response:
  response.send(200, "Hello from Vut")

fn RootRouter() -> Router:
  router:
    Use(Trace)
    Route(path = "/", controller = Home)

fn main() -> int:
  app = vutnet.app()
  app.use(Trace)
  app.router(RootRouter)
  app.listen(3000)
  0
```

Global middleware wraps Router/Routes middleware. Responses unwind in reverse
registration order. A middleware may return its own Response without calling
`next(ctx)` to short-circuit downstream processing. Router mounting uses
`UseRouter(path = ..., router = ...)`; `Use` never mounts a router.

## Static files and multipart

```vut
app.static_files("/assets", "./public")
```

Roots are resolved from process cwd at startup. GET/HEAD serve files with
extension MIME, no directory listing/index fallback. Router paths win; otherwise
the longest static prefix wins. Static responses bypass Vut middleware and
stream files using native file ownership. Invalid mounts return `-7` from
`listen`. `static_files` is used because Vut reserves the keyword `static`.

```vut
fn Upload(ctx: Context) -> Response:
  match ctx.multipart():
    ok(form):
      match form.file("avatar"):
        null: response.send(400, "Missing avatar")
        file: response.send_bytes(200, file.bytes())
    err(_): response.send(400, "Invalid multipart/form-data")
```

`Multipart` exposes `field(name) -> str?`, `fields(name) -> list[str]`,
`file(name) -> MultipartFile?`, and `files(name) -> list[MultipartFile]`.
Files expose `name()`, optional `filename()`/`content_type()`, `bytes()` and
`size()`. Repeated values retain wire order. Uploads remain in memory under the
existing request body bound (default 1 MiB); oversized input returns 413.
Multipart routes retain global/scoped middleware. Filenames are untrusted
metadata and never used to save files. See [the full contract and application](specs/09-static-and-multipart.md)
and `../project/donix/public/index.html` for a browser form.

## Typed View and SSR

Vutnet also provides typed `View` components, layouts/slots, and an HTML5ever
server renderer. A complete standalone VPM app is available in the
[View SSR example](https://github.com/duongonix/vut_repo/tree/main/vpm-package/vutnet-examples/view-ssr).
It renders a composed View from a real Controller and serves the result through
Hyper.

`response.render_hydratable(Page[, data])` adds a versioned typed JSON
bootstrap and shared site/occurrence plan. Vutnet and VPM automatically discover
each statically named Page call-site, generate its typed browser registry, build
WASM plus JS bindings, and serve them for `dev`, `run` and `build`; applications
do not declare a client bin or browser config. The loader uses the bootstrap's
Page ID to call `hydrateView` on existing SSR DOM. Eligible page data gets
compiler-generated `json.Encodable`; a Page data type's static
`from_json`, when present, is automatically used in WASM. Hydrated pages enhance
use client navigation for eligible same-origin links and GET/POST forms, with
Back/Forward support. External links and unsupported responses retain native
browser navigation; POST handlers should use 303 Post/Redirect/Get. See
[`client navigation`](specs/15-client-navigation.md).

Fine-grained bindings use `text(fn() -> str)`,
`bind_attrs(fn() -> list[Attribute])` and scoped `children(fn() -> View)`.
Cell writes evaluate only subscribed callbacks; text/attrs update existing nodes,
and children regions reuse keyed DOM reconciliation. Eager reads outside bindings
retain full-render compatibility. SSR-only `response.render` does not attach
browser events. See [`hydration`](specs/13-ssr-hydration.md) and
[`fine-grained bindings`](specs/14-fine-grained-view.md).

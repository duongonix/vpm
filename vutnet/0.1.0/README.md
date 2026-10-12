# Vutnet

Vutnet is a typed web framework for Vut. It combines a Vut application API and Router DSL with a native Rust HTTP server built on Hyper and Tokio. It provides typed Controllers and request Context, immutable text/bytes/JSON/HTML responses, middleware, safe static serving, multipart parsing, typed Views and optional server-rendered browser hydration.

~~~text
Vut application → typed Router → Controller(Context) → Response
       → Vutnet native adapter → Hyper/Tokio → HTTP client/browser
~~~

The public VPM package is named **vutnet**. Its native HTTP engine is an internal Cargo crate in the same package; there is no separate VPM package named http or server.

## Status and source of truth

This README describes the Vutnet 0.1.x package API. Vutnet uses Vut's typed functions, Receiver Functions, managed values and public Native Callback ABI. Use compatible Vut and VPM versions that satisfy the callback ABI requirements in vpm.toml.

If this document differs from Vut's root language specifications or the compiler/runtime implementation, the implementation is authoritative. Detailed package contracts:

- [Overview](specs/01-overview.md)
- [Router DSL](specs/02-router-dsl.md)
- [App, Context and Response](specs/03-app-request-response.md)
- [Middleware](specs/04-middleware.md)
- [Static files and multipart](specs/09-static-and-multipart.md)
- [View system](specs/10-view-system.md)
- [CLI](specs/11-cli.md)
- [Tailwind](specs/12-tailwind.md)
- [SSR hydration](specs/13-ssr-hydration.md)
- [Fine-grained bindings](specs/14-fine-grained-view.md)
- [Client navigation](specs/15-client-navigation.md)

> **Native distribution:** the current Cargo manifest consumes Vut ABI/SDK/runtime crates through monorepo-relative paths. An isolated registry artifact build must provide these crates in its trusted build context, or the package must move to versioned Rust dependencies. See [package boundaries](specs/06-packages-and-modules.md).

## Install and create an app

VPM builds the package's Vut [[bin]] entry to install the official vutnet command. When your configured registry has this package version and a native artifact for your target, install it with:

~~~sh
vpm install vutnet
vutnet new hello-web
cd hello-web
vutnet dev
~~~

The new command creates a VPM app, pins the Vutnet dependency to the CLI's package version, writes starter Router/Controller source and a comment-only vutnet.toml, then runs vpm update. Tailwind is disabled until you add a [tailwind] section. Existing files are not overwritten. If resolution fails, repair registry settings and run vpm update in the created project.

A VPM app can declare the dependency directly:

~~~toml
[dependencies]
vutnet = "0.1.0"
~~~

Vut function named arguments use equals. Nominal data constructors use colon for fields:

~~~vut
Route(path = "/", controller = HomeController)
HomeData(title: "Welcome")
~~~

## Minimal application

Typical project layout:

~~~text
hello-web/
├── vpm.toml
├── vutnet.toml       # optional
├── src/main.vut
└── public/           # optional assets
~~~

~~~vut
import vutnet
import vutnet.router at router, Router
import vutnet.context at Context
import vutnet.response at Response
import vutnet.response as response

fn HomeController(ctx: Context) -> Response:
  response.send(200, "Hello from Vutnet")

fn RootRouter() -> Router:
  router:
    Route(path = "/", controller = HomeController)

fn main() -> int:
  app = vutnet.app()
  app.router(RootRouter)
  app.listen(3000)
  0
~~~

Run vutnet dev and open http://localhost:3000/. App.listen blocks. The public API currently binds to loopback; it does not accept a bind address or expose a background stop handle.

## CLI commands

The CLI command table is Vut source in this package's [[bin]]. Rust implements OS watching, process control, build orchestration, Tailwind resolution and terminal output behind native bindings. Vutnet is not a separate Rust CLI.

| Command | Behavior |
| --- | --- |
| vutnet new NAME | Create a VPM app with Vutnet configured. |
| vutnet dev | Persistent development supervisor and debug server. |
| vutnet dev --verbose | Include successful static-file requests in logs. |
| vutnet run | Build and run the debug application from the project root. |
| vutnet build | Build a release app/browser client and stage a bundle. |
| vutnet start | Run an existing release bundle without building. |
| vutnet routes | Build/run the app's flattened Router in route-dump mode. |
| vutnet routes --json | Output the route table as JSON. |
| vutnet info | Report project, host, package and available toolchain versions. |
| vutnet --help | Show help. |
| vutnet --version | Show the CLI version. |

Run project commands from the application directory or a subdirectory. Vutnet searches parent directories for vpm.toml. VPM must be available on PATH for commands that build or resolve dependencies.

### Development lifecycle

vutnet dev stays running for the entire session. It watches Vut source, manifests and relevant local path dependencies, debounces save events, and runs only one build transaction at a time:

~~~text
change → compile candidate
            ├─ failure: keep last successful server and wait
            └─ success: stop/reap old child, then start candidate
~~~

Initial compilation failure is recoverable; the watcher remains active. Later build failures leave the last-known-good server and browser assets running. A new server launch failure or runtime crash is logged and the supervisor waits for another source change; it does not enter an infinite restart loop. Ctrl+C cancels the build, stops the watcher, terminates/reaps the child process tree and cleans supervisor artifacts. Terminal history is retained between rebuilds.

Candidate binaries use unique generations under .vut/dev/ to avoid overwriting a running executable on Windows. Changes arriving during a build are coalesced into another build of the latest source.

By default, application requests and failed static-file requests are logged. Successful static responses are quiet. The --verbose option logs every request. Static classification comes from the actual selected handler after Router matching, not a file-extension list. Serving assets is unchanged.

### Build and production start

vutnet run builds a debug application and starts it from the project root. vutnet build creates build/release/vutnet/:

~~~text
build/release/vutnet/
├── app[.exe]
├── manifest.json
└── browser/                 # only when hydratable Pages exist
    ├── client.wasm
    ├── client.js
    ├── client.registry.json
    ├── bootstrap.js
    ├── navigation.js
    └── build-id
~~~

The bundle is staged and validated before publication. vutnet start requires the bundle and does not invoke VPM or rebuild browser assets. Stop a running bundle before rebuilding on Windows. Deploy configured static directories with the app and run start from the project root; static mount paths are relative to that directory.

## Router and Controller

All endpoints use the same typed callable:

~~~vut
type Controller = fn(Context) -> Response
~~~

The alias is in vutnet.router. A wrong arity or result type fails at compile time. Rust retains typed callback handles through the public Native Callback ABI; user code does not use dyn, reflection or raw function pointers.

Import the lowercase router builder and Router type. In router bodies, Route, Routes, Use and UseRouter are receiver methods:

~~~vut
import vutnet.router at router, Router

fn RootRouter() -> Router:
  router:
    Route(path = "/", controller = HomeController)
    Routes(path = "/users"):
      Route(path = "/", controller = UserIndexController)
      Route(path = "/:id", controller = UserController)
~~~

Route defaults to GET. Methods are typed values from vutnet.method:

~~~vut
import vutnet.method as method

Route(
  path = "/users",
  controller = CreateUserController,
  method = @[method.Method.post]
)
~~~

Method supports get, post, put, patch, delete, head and options. Named route parameters use segments such as /users/:id.

Use Routes(path = "..."): to define a lexical prefix. Use UseRouter(path = "...", router = Factory) to mount another fn() -> Router factory. For example, mounting /api, then /admin, then /settings produces /api/admin/settings. Leading/trailing slashes in configured components are normalized and joined with one separator; root remains /. Incoming trailing slashes match a route, but repeated internal slashes are not rewritten. Factories run while configuration is built, not per request. Duplicate method/path and invalid paths fail validation; unmatched paths return 404 and wrong methods return 405.

Use(middleware) registers scoped middleware. UseRouter mounts a RouterFactory. They are separate APIs.

## Request Context

The server builds an owned Context from each request before calling Vut. Controllers do not receive a separate Request object or mutable Response parameter.

| Accessor | Type | Semantics |
| --- | --- | --- |
| method() | str | HTTP method text, such as GET. |
| path() | str | Encoded URL path without the query. |
| param(name) | str? | First route parameter; names are case-sensitive. |
| param_or(name, fallback) | str | Route parameter or fallback. |
| query(name) | str? | First decoded query value. |
| query_or(name, fallback) | str | Query value or fallback. |
| header(name) | str? | First UTF-8 value; lookup is case-insensitive. |
| header_or(name, fallback) | str | Header value or fallback. |
| body_bytes() | bytes | Exact owned request body. |
| body() | result[str, Utf8Error] | Strict UTF-8 conversion. |
| multipart() | result[Multipart, MultipartError] | Parsed snapshot or typed error. |
| client_ip() | str | Direct TCP peer address. |
| client_port() | int | Direct TCP peer port. |
| request_id() | int | Correlation ID within this server run. |

Query decoding follows form-style rules, where plus becomes space. Repeated query pairs are retained but query_all is not yet exposed. Invalid UTF-8 headers or invalid request escapes fail before Controller dispatch. Forwarded headers are not trusted; client_ip/client_port are not proxy-aware. request_id is not a security token.

Request bodies are bounded before Vut dispatch. Default limit is 1 MiB; excess returns 413. App.with_max_body_bytes(nonnegative_limit) returns result[App, AppConfigError]; zero accepts empty bodies only.

## Response

Responses are immutable values created in vutnet.response and returned from the Controller:

~~~vut
import vutnet.context at Context
import vutnet.response at Response
import vutnet.response as response

fn HomeController(ctx: Context) -> Response:
  response.send(200, "Hello")
~~~

| Constructor | Input | Content-Type |
| --- | --- | --- |
| response.send(code, text) | int, str | text/plain; charset=utf-8 |
| response.send_bytes(code, bytes) | int, bytes | application/octet-stream |
| response.html(code, html) | int, str | text/html; charset=utf-8 |
| response.json(code, value) | int, json.Value | application/json; charset=utf-8 |
| response.json_encodable(code, model) | int, json.Encodable | application/json; charset=utf-8 |

send_bytes preserves exact bytes; Vut does not implicitly convert bytes to str. response.html should receive View renderer output or other trusted HTML. The compiler synthesizes json.Encodable.to_json() for eligible non-generic data records with supported JSON fields. A type can implement to_json() for a custom wire shape. Arbitrary values are not serialized through reflection.

Response.with_header(name, value) returns result[Response, HeaderError]. It validates HTTP field syntax and returns a new Response, replacing any matching header name case-insensitively. Invalid status/header data becomes a deterministic server error. There is no mutable res.status/res.header/res.text facade or multi-value append API.

## Middleware

~~~vut
type Next = fn(Context) -> Response
type Middleware = fn(Context, Next) -> Response
~~~

Register globally with app.use(Middleware), or in a Router/Routes scope with Use(Middleware). Global middleware runs outside scoped middleware. Applicable middleware runs before the Controller in scope order and unwinds in reverse order. A middleware may short-circuit by returning without calling next(ctx). V1 has no one-shot guard, so middleware should call next(ctx) at most once. Static responses and framework-level 404/405 bypass Vut middleware.

~~~vut
import vutnet.context at Context, Next
import vutnet.response at Response
import vutnet.response as response

fn Trace(ctx: Context, next: Next) -> Response:
  result = next(ctx)
  match result.with_header("x-trace", "complete"):
    ok(updated): updated
    err(_): response.send(500, "Invalid header")
~~~

The middleware API is described in [specs/04-middleware.md](specs/04-middleware.md).

## Static files

Mount a directory explicitly:

~~~vut
app.static_files("/assets", "./public")
~~~

Roots resolve from the project working directory at startup. Thus /assets/site.css maps to public/site.css. A root mount can serve public at URL root.

Router matches take priority, including a 405; otherwise the longest segment-boundary static prefix handles the request. Only GET and HEAD open files. HEAD returns metadata and content length without a body. MIME type is inferred from extension, falling back to application/octet-stream.

There is no directory listing, index.html fallback, range support, conditional cache, compression or public streaming Response. Missing paths return 404, unsupported methods 405. The static handler rejects traversal, encoded separators, device names and symlink/reparse escapes using capability-based filesystem access. Static responses bypass middleware. See [static and multipart details](specs/09-static-and-multipart.md).

## Multipart uploads

Context.multipart() parses once into an owned snapshot. Accessors:

~~~text
Multipart.field(name)         -> str?
Multipart.fields(name)        -> list[str]
Multipart.file(name)          -> MultipartFile?
Multipart.files(name)         -> list[MultipartFile]
MultipartFile.name()          -> str
MultipartFile.filename()      -> str?
MultipartFile.content_type()  -> str?
MultipartFile.bytes()         -> bytes
MultipartFile.size()          -> int
~~~

Repeated values preserve wire order; field/file return the first match. Text fields require UTF-8; file bytes remain binary. Filename and content type are untrusted metadata. Vutnet never writes uploads to disk or uses a submitted filename as a path. Uploads stay in memory under the request body bound and finite parser part limit. Malformed input returns MultipartError for application policy; oversized bodies receive 413 before Controller dispatch.

## View components and server rendering

View is the type; lowercase view(...) builds one. HTML tags and text are typed Receiver Function methods available inside the View body; they are not imported individually. Any Vut function returning View is a component and can be called directly:

~~~vut
import vutnet.view at View, view
import vutnet.context at Context
import vutnet.response at Response
import vutnet.response as response

fn Greeting(name: str) -> View:
  view:
    p:
      text("Hello, $name")

fn HomePage() -> View:
  view:
    html(lang = "en"):
      head:
        link(href = "/assets/app.css", rel = "stylesheet", link_type = "text/css")
        title:
          text("Vutnet app")
      body:
        main(class = "page"):
          h1:
            text("Welcome")
          Greeting("Vut developer")
~~~

response.render(Page) accepts fn() -> View. response.render(Page, data) accepts fn(T) -> View with a value of exactly T; a mismatch is a compile error. Both call the typed Page and reuse the same layout/slot resolver and HTML5ever-based serializer. Render/layout errors produce HTTP 500 with a generic client body and detailed server diagnostic. views.render(View) remains the low-level HTML API; response.html is for already-rendered trusted HTML.

### Layout, slot and typed data

Layouts are ordinary functions returning View. A Page chooses the layout with view(layout = SiteLayout):; the Controller does not select it. A slot with a receiver body supplies fallback content. The Page supplies content under the same slot name:

~~~vut
import vutnet.view at View, view
import vutnet.context at Context
import vutnet.response at Response
import vutnet.response as response

fn SiteLayout() -> View:
  view:
    html(lang = "en"):
      body:
        header:
          slot(name = "header"):
            h1:
              text("My site")
        main:
          slot(name = "content"):
            p:
              text("Fallback")

data DocsData:
  title: str

fn DocsPage(data: DocsData) -> View:
  view(layout = SiteLayout):
    slot(name = "header"):
      h1:
        text(data.title)
    slot(name = "content"):
      h1:
        text(data.title)

fn DocsController(ctx: Context) -> Response:
  response.render(DocsPage, DocsData(title: "Documentation"))
~~~

Unfilled fallback slots render their fallback. Required/duplicate/unknown slots and unresolved nested layouts are render errors. Views support multiple roots, fragments, if/else, for loops and components.

### HTML elements, attributes and CSS

Typed tag methods cover document/head tags (html, head, body, title, meta, link, style, script), semantic sections, headings/text, links/images, lists, form controls, tables, media and iframe. The complete tag/shortcut list is in [specs/10-view-system.md](specs/10-view-system.md).

Shortcut parameters are typed: boolean attributes take bool; integer attributes use int?; numeric input constraints use float?; text attributes use str?. Keyword aliases include input_type, button_type, link_type, for_id and async_enabled. They serialize to standard HTML names. False boolean attributes are omitted; true attributes use HTML boolean serialization.

Custom/data/ARIA attributes use attrs = @[attr(name, value)]. attr takes positional arguments:

~~~vut
import vutnet.view at View, view, attr

fn Profile() -> View:
  view:
    div(
      id = "profile",
      class = "card",
      attrs = @[
        attr("role", "region"),
        attr("data-page", "profile"),
        attr("aria-label", "Profile")
      ],
      css = (
        "display": "grid",
        "gap": "1rem"
      )
    ):
      text("Hello")
~~~

The css option is map[str, str], where each pair is one property and value. It follows the same Attribute validation and serialization path; property names are sorted for deterministic output. CSS syntax validation is not a sanitizer and does not promise browser support. SSR escapes text/attribute values, validates supported attributes and rejects unsafe URL forms. View does not mount public files; use app.static_files.

### Typed browser events

Event handlers are typed callbacks, never inline JavaScript strings. The browser bridge copies only the declared snapshot fields; no raw JS Event or DOM object enters Vut.

| Event | Callback type | Useful fields |
| --- | --- | --- |
| on_click | MouseEvent | target, button, modifier keys |
| on_input | InputEvent | target, value, checked |
| on_change | ChangeEvent | target, value, checked |
| on_key_down / on_key_up | KeyboardEvent | target, key, code, repeat |
| on_focus / on_blur | FocusEvent | target |
| on_submit | SubmitEvent | target |
| on_invalid / on_reset / on_select | Event | target |

Handlers vary by tag: button supports click/focus/blur/key; input supports input/change/focus/blur/key/click/invalid; textarea supports input/change/focus/blur/key/select/invalid; select supports change/input/focus/blur/invalid; form supports submit/reset/invalid; label supports click; option may support click while selection changes belong to select.on_change. Full mapping is in the View specification.

event.prevent_default() and event.stop_propagation() act on the original DOM event only during its synchronous Vut callback. Listener registration is owned by the mounted View. Refresh replaces old listeners; unmount removes listeners and releases callback resources.

## Browser state, hydration and reactive updates

response.render is SSR only. Use response.render_hydratable(Page[, data]) to SSR a Page and attach the browser client:

~~~vut
import vutnet.web at state
import vutnet.view at View, view
import vutnet.context at Context
import vutnet.response at Response
import vutnet.response as response

data CounterInitial:
  count: int

fn CounterPage(initial: CounterInitial) -> View:
  count = state(initial.count)
  view:
    main:
      h1:
        text(fn() -> str:
          "Count: $(count.get())"
        )
      button(on_click = fn(event):
        count.set(count.get() + 1)
      ):
        text("+")
      button(on_click = fn(event):
        count.set(count.get() - 1)
      ):
        text("-")

fn CounterController(ctx: Context) -> Response:
  response.render_hydratable(CounterPage, CounterInitial(count: 7))
~~~

Current overloads accept fn() -> View or fn(Data) -> View. Pack multiple values into one nominal data record. Hydration data is statically typed and must implement json.Encodable. Use data constructor colons, for example CounterInitial(count: 7). Generic, variadic or dynamically computed Page callables are not supported by the generated client registry.

VPM generates a typed client registry for each statically named hydratable Page. vutnet dev, run and build automatically build the WASM client and stage JavaScript bindings, registry, bootstrap and navigation runtime. Apps do not declare a browser section, client [[bin]], factory or JSON conversion boilerplate. Eligible data records get generated encoding/decoding; a public static Data.from_json can customize decoding.

The browser loader validates the typed bootstrap/shared hydration plan and calls hydrateView on existing SSR DOM. Initial hydration does not replace the document or rebuild the Page. State cells use vutnet.web.state(value), returning Vut's identity-bearing cell[T]; read with get() and write with set(value). Helpers include state_get, state_set, state_update and state_revision. State payloads must satisfy Vut's CellSafe rules. Mutating a value held inside a cell does not itself notify bindings; write the changed value through the cell.

Fine-grained bindings:

- text(fn() -> str) updates one existing text node.
- bind_attrs(fn() -> list[Attribute]) updates one element's attributes.
- children(fn() -> View) reconciles one element's child region; use it for reactive conditions, loops and nested components.

Only callbacks subscribed to changed cells are reevaluated; updates coalesce to a browser animation frame. Child regions use keyed DOM reconciliation. Give reorderable item roots unique, stable HTML ids to preserve identity. Removed nodes clean up listeners/subscriptions. Binding callbacks should be pure because they run during SSR, browser construction/attachment and later updates.

SSR reads the same initial value and does not need browser runtime. Without JavaScript, HTML remains visible, but events and reactive updates are unavailable. See [hydration](specs/13-ssr-hydration.md) and [fine-grained bindings](specs/14-fine-grained-view.md).

## Client navigation

Hydratable pages progressively enhance eligible same-origin links/forms. Normal anchors and forms work as full document requests when JavaScript is unavailable. With the client loaded, an unmodified primary click on an eligible same-origin HTTP(S) link fetches destination SSR, disposes the old mount and hydrates the destination without replacing window or the document root. The existing server Router remains the only Router.

Back/Forward uses History API. A destination gets its own typed bootstrap and fresh Page-local state; old route cells do not transfer. Modified clicks, downloads, new-window targets, external links, fragments and unsupported responses use native navigation. GET forms serialize successful controls. POST supports URL-encoded and multipart forms; use a 303 redirect after successful writes so refresh makes a normal GET. Unsupported direct POST responses are not automatically replayed. Prefetch is opt-in with rel=prefetch and requires positive Cache-Control max-age.

Responses with CSP, executable page scripts or missing/mismatched hydration metadata fall back to full navigation where safe. Navigation lifecycle events are dispatched on document as vutnet:navigationstart, vutnet:navigationend and vutnet:navigationerror. Pending state is reflected in data-vutnet-navigation-state on html. See [client navigation](specs/15-client-navigation.md).

## Tailwind CSS (optional)

Vutnet invokes the standalone Tailwind CSS CLI without Node.js, npm or npx. Configure the project-root vutnet.toml:

~~~toml
[tailwind]
version = "4.1.14"
input = "src/app.css"
output = "public/assets/app.css"
~~~

CSS input:

~~~css
@import "tailwindcss";
~~~

The presence of [tailwind] enables integration. Version must be exact and catalogued; ranges and latest are rejected. Paths are project-relative and cannot escape the project. Link the output from a View and mount it:

~~~vut
app.static_files("/assets", "./public/assets")
~~~

In the View head, emit the CSS link with the typed tag API:

~~~vut
head:
  link(href = "/assets/app.css", rel = "stylesheet", link_type = "text/css")
~~~

Tailwind performs class discovery. If automatic source detection misses Vut files, add the corresponding Tailwind @source directive in the CSS input. Vutnet does not rewrite CSS or scan classes.

On first use, Vutnet asks VPM to resolve the exact standalone host executable. VPM verifies and caches the tool; Node/npm/npx are not required. Resolution happens once per run/build command and once per dev session, then the same binary is reused on rebuilds. A cache miss can show download percentage progress. Without [tailwind], no tool lookup occurs.

For run/build, CSS is built as a temporary candidate and published only after the app build succeeds. During dev, CSS and application assets are one generation; a failure keeps the last-known-good generation. Restart dev after editing vutnet.toml because config/tool version are read at startup. See [Tailwind integration](specs/12-tailwind.md).

## Logging and diagnostics

The native terminal UI prints a compact startup banner, local URL, mode, watch path and aligned request summaries. ANSI colors adapt to TTY, are disabled for redirected output, and honor NO_COLOR. The dev supervisor appends rebuild output without clearing the terminal. Successful static requests are hidden by default; application requests and failed static requests remain visible, and --verbose prints all requests.

Internal request failures have a separate error block with method/path, status, duration, failure stage and safe diagnostic. The client receives a generic 500 body. Request bodies, headers and query values are not logged by default. Compiler/build diagnostics come from VPM/CLI.

## Limits and security

- Listener currently binds to loopback; App.listen has no configurable public bind address.
- Hyper accepts multiple connections, while Native Callback ABI v1 serializes Vut callback execution. Foreign-thread Vut entry is not exposed.
- Unhandled Vut callback traps still follow runtime abort behavior; they cannot currently be caught and mapped to HTTP 500.
- Default body limit is 1 MiB. Multipart file data remains in memory within that per-request limit and parser part bound.
- Static serving supports GET/HEAD with path confinement/MIME inference, but no index fallback, byte ranges, compression or public streaming Response.
- Context exposes first-value query/header accessors only. All-values, proxy trust and TLS metadata are not public.
- response.render is SSR only. Events, reactive state, hydration and client navigation require response.render_hydratable.
- Hydration registry requires statically named supported Pages. Native Callback ABI v1 promotes named/non-capturing callables; browser capture support depends on the WASM callable/runtime path.
- No template engine, vut.render, WebSocket API, TLS abstraction, public streaming Response or programmatic server-stop handle is included in 0.1.x.

## Develop from the Vut repository

The monorepo keeps this package under vpm-package/vutnet/, ignored by the root Cargo repository. With Vut and VPM configured, run checks from the Vutnet package directory:

~~~sh
vpm check
vpm build
vpm test
~~~

Standalone HTTP/View consumer examples live outside the publishable package in vpm-package/vutnet-examples/ so their nested VPM manifests are not archived. Implementation and acceptance evidence are in specs/07-roadmap-testing-rules.md.

## License

Vutnet is distributed under the MIT License. See [LICENSE](LICENSE).

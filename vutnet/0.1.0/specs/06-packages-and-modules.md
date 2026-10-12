# Vutnet — Packages and Modules

## 1. Package boundaries

The public VPM package is vutnet. Its Vut modules define App, Context, Response,
HTTP Method and the Router DSL.
It also owns static mount configuration, typed multipart accessors, and the
typed View/layout/SSR API.

HTTP server infrastructure is an internal Rust Cargo crate named
`vutnet-server-native` at `native/server` inside the `vutnet` VPM package. It
owns real sockets, Hyper/Tokio, connection lifecycle, bounded request bodies,
request/response transport and server errors. The Vutnet adapter and server
engine remain separate Rust crates for clean responsibilities, but they are
published and versioned together as one VPM package. The server engine does not
own the Router DSL, Controller, RouterFactory, Context, middleware or
application-framework behavior.

The official command-line executable belongs to the `vutnet` VPM package. Its
`[[bin]]` points to `src/bin/vutnet.vut`; command selection and argument
validation are written in `src/cli.vut`. System-level command work is Rust
inside `native/src/cli/`, reached through typed managed native imports in the
same native library. Do not add the CLI to Vut core or add a separate Rust
executable/package.

Do not name a package `http`: Vut already has a standard-library module with
that name for its outgoing HTTP client. The internal Cargo crate is named
`vutnet-server-native`; it is not a VPM child package or public Vut namespace.

MVP does not need a separate `net` package. Tokio supplies networking/runtime
infrastructure and Hyper supplies HTTP server protocol inside the internal
server crate. Keep the VPM release as one package unless a real separately
versioned Vut API boundary emerges.

## 2. Current package layout

~~~text
vpm-package/vutnet/
├── AGENTS.md
├── README.md
├── vpm.toml
├── vpm.lock
├── specs/
│   ├── 01-overview.md
│   ├── 02-router-dsl.md
│   ├── 03-app-request-response.md
│   ├── 04-middleware.md
│   ├── 05-native-rust.md
│   ├── 06-packages-and-modules.md
│   ├── 07-roadmap-testing-rules.md
│   ├── 08-server-package.md
│   ├── 09-static-and-multipart.md
│   └── 10-view-system.md
├── src/
│   ├── mod.vut
│   ├── cli.vut
│   ├── bin/
│   │   └── vutnet.vut
│   ├── method.vut
│   ├── context.vut
│   ├── response.vut
│   ├── static_files.vut
│   ├── multipart.vut
│   ├── view/
│   │   └── mod.vut
│   ├── server.vut
│   └── router/
│       ├── mod.vut
│       └── path.vut
├── native/
│   ├── build.toml
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── build.rs
│   ├── src/
│   │   ├── lib.rs
│   │   ├── cli.rs
│   │   ├── cli_ffi.rs
│   │   ├── cli/
│   │   │   ├── commands/
│   │   │   │   ├── dev.rs
│   │   │   │   └── dev/
│   │   │   │       ├── artifact.rs
│   │   │   │       ├── builder.rs
│   │   │   │       ├── runtime.rs
│   │   │   │       └── supervisor.rs
│   │   │   ├── process.rs
│   │   │   ├── project.rs
│   │   │   ├── toolchain.rs
│   │   │   ├── ui.rs
│   │   │   └── watcher.rs
│   │   ├── identity.rs
│   │   ├── ffi.rs
│   │   ├── routing.rs
│   │   ├── request.rs
│   │   ├── context.rs
│   │   ├── response.rs
│   │   ├── lifecycle.rs
│   │   ├── static_files/
│   │   ├── multipart/
│   │   └── view/
│   └── server/
│       ├── Cargo.toml   member of the native workspace
│       ├── src/          internal Tokio + Hyper transport crate
│       └── tests/        real-socket HTTP engine tests
└── tests/
    ├── server_link.vut
    ├── router_basic.vut
    ├── router_composition.vut
    ├── router_paths.vut
    ├── router_validation.vut
    ├── router_factory.vut
    └── router_import_boundary.vut
~~~

The Vut source and tests contain Router, middleware, static/multipart and View
implementations. The Vutnet native adapter under `native/` links the internal
`native/server` crate; HTTP integration is covered by
`crates/vpm/tests/vutnet_http_e2e.rs` and View SSR by
`crates/vpm/tests/vutnet_view_http_e2e.rs`. Standalone runnable VPM examples
live outside the publishable package in `vpm-package/vutnet-examples/` because
VPM rejects nested project manifests during publication.
Keep response construction in `response.vut`, Context in `context.vut`, `Method`
in `method.vut`, and the public Router API plus its path helper in `router/`.
The internal server crate keeps its Rust code split by server, connection,
transport, body and lifecycle responsibilities.

`static_files/` owns filesystem security/mount resolution; `multipart/` owns
bounded parsing and nominal snapshot building. The internal HTTP engine only
provides generic opened-file streaming; it knows no static URL policy or Vut
multipart types. Crates: cap-std, mime_guess, mime and multer in the adapter;
tokio-util/futures-util plus Tokio fs support in `native/server`. Donix's `public/` and
upload controller demonstrate both features; the main acceptance test is
`crates/vpm/tests/vutnet_static_multipart_e2e.rs`.

## 3. One VPM package, separate Rust responsibilities

`vpm.toml` has no local VPM dependency. The server engine is an internal Cargo
crate referenced by the Vutnet native manifest:

~~~toml
# vpm-package/vutnet/native/Cargo.toml
[dependencies]
server-native = { package = "vutnet-server-native", path = "server" }
~~~

Cargo resolves that relative path from `native/Cargo.toml`; it remains inside
the Vutnet package. This keeps the adapter and HTTP engine modular without a
second VPM package, nested manifest, or separately released server version.

Dependency direction:

~~~text
application
    ↓
vutnet public Vut API
    ↓
vutnet native adapter (route table + Context/Response mapping)
    ↓
internal vutnet-server-native Cargo crate (Hyper/Tokio transport)
    ↓
Hyper + Tokio
    ↓
OS / network
~~~

The `vutnet` package's root `native/build.toml` invokes the adapter Cargo
manifest. Cargo builds the internal server crate as its dependency, and VPM
caches/links the resulting native artifact once. Consumers do not configure
archive paths and do not resolve a separate `server` package.

## 4. Native build and publish

VPM publication rejects local VPM path dependencies and nested `vpm.toml`
projects. The package therefore owns one VPM manifest and one native build
entrypoint. Runnable examples are maintained in the sibling
`vpm-package/vutnet-examples/` directory, outside the package archive.

`vpm check` type-checks without a native build. Registry publication still uses
the trusted `native/build.toml` -> `native-artifacts.toml` pipeline. The package
has no local VPM dependency or nested `vpm.toml`; however, `native/Cargo.toml`
uses repository-relative paths to `crates/vut-native-abi`, `crates/vut-native-sdk`
and the `vut-runtime` dev dependency. An isolated source archive cannot build
these paths. Before publishing, provide a supported reproducible build context
that includes those root crates or move them to versioned Rust dependencies.

The generic VPM native lifecycle is defined by `specs/vpm/native-packages.md`.

## 5. Import syntax

Vut's canonical module imports are whole-module import, module alias, or
selected imports. A single statement must not combine as and at. Public Vutnet
modules follow the source-root namespace:

~~~vut
import vutnet
import vutnet.context at Context, Next, Middleware
import vutnet.response at Response
import vutnet.response as response
import vutnet.router at router, Router, Controller, RouterFactory, RouteEntry, RouterError, Use, UseRouter
import vutnet.method as method
~~~

or the whole-module alias form:

~~~vut
import vutnet.router as router
~~~

Both forms are exercised by `tests/router_import_boundary.vut` and the
compile-fail suite. Receiver methods are visible across module and local VPM
package boundaries
(proven by `crates/vpm/tests/receiver_package_e2e.rs` and
`crates/vut-compiler/tests/receiver_module_e2e.rs`), so `App.router`,
`App.listen`, `App.use`, the Context accessors and the Router `Route`/`Use`/
`UseRouter`/`Routes` methods resolve from a consuming application module. The real-socket
`vutnet_http_e2e` test also compiles and executes this consuming application.
`Routes` groups are supported.

The low-level server engine is an internal Cargo crate linked by Vutnet; it is
not a VPM package and application code does not import it. The legacy scalar
`vutnet.server.serve_fixed` proof entrypoint remains covered by its integration
fixture; the engine boundary is documented in 08-server-package.md.

Vutnet consumers commonly keep Router and Controller definitions in local
application files, so:

~~~vut
import router
import controller at AdminController, SettingController
~~~

is also valid when router.vut and controller.vut are in the application's
source root. response.send is a module function imported through the response
module alias. Do not use the nonexistent std namespace path std.json; standard
library JSON is imported as json.

## 6. Desired public and internal modules

Public surface (source implementation status):

- vutnet: `app`, `App.router`, `App.use`, `App.with_max_body_bytes` and `App.listen`;
  the listener drains connections on process shutdown signals;
- vutnet.context: Context, `Next`, `Middleware` and implemented request accessors;
- vutnet.response: Response, `send`, `send_bytes`, `html`, `json`,
  `json_encodable` and validated immutable `Response.with_header`;
- vutnet.view: `View`, typed tag/attribute builders, `slot`, `fill`, `resolve`
  and managed `render(View) -> result[str, str]`;
- vutnet.view.events: typed `ClickEvent`, `ClickHandler`, separate `EventBinding`,
  validated `InteractiveView`, and in-process Vut dispatch; no DOM bridge yet;
- vutnet.method: typed Method values;
- vutnet.router: Router, RouterScope, RoutesScope, RouteEntry, RouterError,
  RouterFactory, Controller, `router`, `Route`, `Routes`, scoped middleware `Use`,
  `UseRouter` mounts, `Router.entries` and `Router.flatten`.

Internal surface:

- vutnet.router.path: path normalization helpers (relative import only);
- vutnet.router.RouterBuilder: shared accumulation value; public name only
  because Vut cannot construct a `_`-prefixed type, private field, unsupported;
- `vutnet/src/mod.vut`: typed native import declaration for the flattened
  route list; the compiler selects public callback-ABI marshalling because the
  RouteEntry type graph contains its typed DispatchPipeline callable;
- `vutnet/native`: Rust adapter for route loading, Context/Response mapping,
  typed callback invocation and View SSR over public ABI/SDK values;
- server: low-level native Hyper/Tokio core, not needed by normal application
  code;
- server/src/mod.vut: scalar native entrypoints and the phase-2 facade for
  `server`;
- Rust handle IDs, body buffers, route-table serialization and callback
  bookkeeping.

Do not expose pointers, callback IDs, Hyper types, Tokio types or Rust
lifetimes as ordinary public Vut concepts.

## 7. Core modification policy

The general typed list/callable-field callback import needed by Vutnet has been
implemented in Vut core. The current checker selects that import path when a
managed parameter's type graph contains a callable. A plain managed native
import with no callable in its parameter graph remains subject to direct C ABI
rules and can still fail E8004; this does not block the Vutnet route table.

If a future Vutnet feature needs another managed import shape:

1. preserve the typed Vut API in the design;
2. create a minimal compiler/native reproduction;
3. identify whether the limitation is compiler, runtime, stdlib or VPM;
4. propose a general public fix;
5. stop dependent implementation and request approval.

Do not silently weaken Controller to dyn or expose raw pointers to users.

Vut evaluates and flattens the RouterFactory before crossing the boundary; the
adapter receives the typed list once at startup and retains callback handles
for request-time dispatch. The real socket proof is recorded in
07-roadmap-testing-rules.md.

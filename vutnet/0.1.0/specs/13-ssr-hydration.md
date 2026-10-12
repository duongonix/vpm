# SSR Hydration — Bootstrap and Shared Plan

Status: **steps 1–6 are implemented**: typed SSR bootstrap, shared call-site
plan, initial DOM reuse, typed event/state attachment, dynamic-tree validation,
mismatch handling, mount cleanup, browser assets and same-origin client navigation.
Initial hydration preserves SSR DOM. Later state writes reevaluate the View and
patch matching DOM nodes, preserving document resources and controls.

## Goals and boundaries

Hydration lets the browser run the same statically typed Vut Page that the server
rendered, validate the existing HTML, then attach callbacks and state
subscriptions without replacing the initial DOM. It reuses the existing View
builder, call-site plan, typed callback bridge and state subscription lifecycle.

The server and browser must agree on request-specific data and on the evaluated
View tree. No native or WASM object layout, address, raw pointer, private
Runtime Handle, untyped callback, reflection, or `map[str, dyn]` crosses this
contract. Ordinary `response.render` and `views.render` keep their current
unmarked behavior.

## Findings from the implementation

- Vutnet builds `View` as a flat preorder `list[ViewNode]`; `child()` and
  layout/slot resolution flatten components before the Page returns.
- View tag/text builders use Vut's general `@track_caller` and
  `caller_call_site_id() -> u64`. IDs are compiler-derived and identical for
  native and WASM builds of the same logical source.
- `View._assign_occurrences` runs after the final tree is composed and delegates
  to `view.hydration._occurrence_plan`. It creates a typed `_hydration_plan` in
  the same order as `_nodes`, assigning a 0-based occurrence number per static
  site. Each entry contains site ID, occurrence, parent index, node kind, and tag
  name.
- Native SSR receives the callback-free `ViewRender` projection through the
  public managed Native SDK. It validates that each plan entry matches its
  corresponding node, then serializes HTML through the existing HTML5ever
  renderer. Browser WASM receives the same `View` fields; no compiler/backend
  special case was added for Vutnet.
- Vutnet does not ask application code to write JSON codecs for ordinary page
  data. When the standard `json` module participates in a compilation, the
  compiler synthesizes `to_json() -> json.Value` for eligible non-generic,
  non-opaque `data` declarations whose fields have supported JSON shapes
  (scalars, lists/arrays, and nested encodable data). Browser marshalling uses
  the Page's compiled nominal field schema. A public static `Data.from_json`,
  when present, is automatically wrapped into that Page's generated client
  decoder. This is compile-time typed generation, not runtime reflection.

## Step 1 — Typed SSR-to-WASM bootstrap (DONE)

The high-level opt-in API is:

~~~vut
response.render_hydratable(HomePage)
response.render_hydratable(DocsPage, docs_data)
~~~

The overloads are statically typed as:

~~~text
render_hydratable(page: fn() -> View) -> Response
render_hydratable[T: json.Encodable](page: fn(T) -> View, data: T) -> Response
~~~

`DocsPage: fn(DocsData) -> View` only accepts `DocsData`. For eligible `data`
types the compiler supplies the `json.Encodable` implementation automatically;
application code only declares the nominal fields and constructs the value. A
custom `to_json()` can still be written for a deliberate wire representation.
The Page is invoked once on the server; layout and slots resolve through the
existing View builder before SSR. A complete ordinary page-data example needs no
user-authored `to_json` or `from_json` methods:

```vut
# src/data/todo.vut
data ClientInitial:
  page: str
  count: int

# src/views/todo.vut
import vutnet.view at View, view, h1, text
import data.todo at ClientInitial

fn TodoPage(initial: ClientInitial) -> View:
  view:
    h1:
      text("Todo App $(initial.count)")

# src/controllers/todo.vut
import vutnet.context at Context
import vutnet.response at Response
import vutnet.response as response
import data.todo at ClientInitial
import views.todo at TodoPage

fn TodoController(ctx: Context) -> Response:
  response.render_hydratable(
    TodoPage,
    ClientInitial(page: "todo", count: 0)
  )
```

No-data Pages carry `has_data: false` and JSON `null`. Normal
`response.render(Page[, data])` remains unchanged and does not add markers or
bootstrap data.

The native renderer appends one inert script element to the document body when
available (otherwise after the root nodes):

~~~html
<script type="application/json" data-vutnet-bootstrap="v2">…</script>
~~~

Its JSON envelope identifies the Page call-site as well as initial data:

~~~json
{
  "protocol": "vutnet.hydration",
  "version": 2,
  "page_id": "v1-0123456789abcdef",
  "has_data": true,
  "data": { "title": "Documentation" },
  "plan": {
    "version": 2,
    "nodes": [
      { "site": "v1-0123456789abcdef", "occurrence": 0, "parent": -1, "kind": "element", "name": "main" }
    ]
  }
}
~~~

`data` is produced through the ordinary `Encodable.to_json()` contract, which
the compiler synthesizes for eligible data declarations or application code may
customize. The browser entry point selects a typed `fn(T) -> View` and marshals
the JSON object field-by-field using that function's generated nominal `T`
schema. It checks field value types and canonical nominal identity when a tagged
value is provided. The default path does not invoke `json.Decodable`; bootstrap
JSON must match the Page parameter's nominal fields. A configured custom decoder
is only needed for a custom wire format.

The native renderer parses and reserializes the JSON with `serde_json`, escapes
`<`, `>`, `&`, U+2028, and U+2029 before placing it in script raw text, and
rejects payloads whose complete envelope exceeds 1 MiB. User View attributes
cannot forge `data-vutnet-bootstrap` when this API is used. The bootstrap is
public page data: applications must not include secrets that should remain
server-side.

## Step 2 — Shared call-site plan (DONE)

The plan is based on two parts:

1. **Static site identity:** the compiler-generated Vut call-site ID, encoded
   with the root call-site identity version (`v1-` plus 16 lowercase hex
   digits). It excludes absolute paths, source load order, `SourceId`, runtime
   addresses, and target-specific layouts.
2. **Runtime occurrence:** a 0-based counter for each site ID in final preorder.
   This distinguishes repeated evaluations of one call expression in loops or
   repeated component instances for the initial server/browser tree.

The full `View._hydration_plan` sequence contains the site ID, occurrence,
parent node index, `text`/`element` kind, and tag name. It is computed in Vut
after component and layout/slot flattening and is carried unchanged into the
callback-free native SSR projection. Browser WASM sees the same typed plan on
the returned View. The dynamic node sequence is request-specific; the static
site identities inside it are compile-time values.

Opt-in SSR markers use marker format version 2:

- element: `data-vut-site="v2-<16-hex-site-id>-<occurrence>"`;
- ordinary text: `<!--vut-site:v2-<16-hex-site-id>-<occurrence>-->` immediately
  before the text node.

`script`, `style`, `title`, and `textarea` text has no comment marker because
HTML parses comment syntax there as content. Its plan entry is still included
in the bootstrap, so the future matcher can validate it by its parent and text
position. Unresolved slots remain render errors. The `data-vut-site` attribute
is reserved in hydratable output.

The native side checks plan length, site identity, parent, kind, tag, and
per-site occurrence order against `ViewRenderNode` before serialization. This
keeps the bootstrap plan and HTML markers tied to the same validated View.
Plan version 2 means the marker now includes an occurrence; it is independent
from bootstrap envelope version 1 and call-site encoding version 1.

The compiler does not statically execute arbitrary Page functions or infer
request-time branches and loops. Therefore the plan is an evaluated tree built
from compile-time site IDs, not a separate ahead-of-time HTML manifest. This
avoids adding a Vutnet-specific compiler pass while keeping the same plan
available to both targets.

Occurrence identity validates the initial tree; it is not a persistent list key.
For updates, a unique HTML `id` is an explicit stable node key. Other nodes
match by compile-time site plus local sibling occurrence under the retained
parent. Give repeated component/list roots stable IDs when items can reorder.
Without IDs, repeated siblings use positional identity; insertion/reordering
cannot preserve their per-item identity. No new key syntax or ABI is introduced.
Keys must be unique across the mounted View and stable for the lifetime of an
item, including across component boundaries. Duplicate IDs reject the candidate
before live DOM mutation. Removing an item disposes its listeners; inserting
the same ID on a later refresh creates a fresh node, not a resurrected instance.

Example: give the component root an item-derived ID; its unkeyed children
remain scoped to that retained root during insertion, deletion and reordering.

~~~vut
fn ItemRow(item: str) -> View:
  view:
    li(id = "item-$item"):
      input(value = item)

fn ItemsPage(items: list[str]) -> View:
  view:
    ul:
      for item in items:
        ItemRow(item)
~~~

## Step 3 — Hydrate existing DOM (DONE)

The generated browser binding exposes:

~~~typescript
hydrateView(container: Element, page: () => View, options?: VutHydrationOptions): VutMountedView
hydrateView<T extends object>(container: Element, page: (data: T) => View, options?: VutHydrationOptions): VutMountedView
~~~

The Page is invoked once synchronously with no data or with bootstrap JSON
marshalled through the Page's static nominal parameter schema. Hydration then
validates the complete evaluated tree against both the bootstrap plan and the
existing DOM before attaching any event listener or state subscription. The
matcher checks plan version, static site identity, occurrence, parent index,
node kind, tag name, markers, order, attributes and text content. Raw-text
elements use their plan position and text content because HTML parsing does not
preserve comments there.

On success, the host maps plan indices to existing Element/Text nodes and
attaches listeners/subscriptions through the same lifecycle used by `mountView`;
initial hydration does not call `replaceChildren` and preserves node identity.
Each container can have one active mount. The document must contain exactly one
`data-vutnet-bootstrap="v2"` script.

## Step 4 — Dynamic hydration correctness (DONE, bounded)

The plan is computed from the evaluated View after `if/else`, `for`, component,
nested component, layout and slot composition. Hydration validates the resulting
tree rather than trying to infer source-level branches. Text and attribute
values must match the server result at initial attach. When a state cell changes
after hydration, the existing state runtime reruns the View on the next frame
and commits incremental text/attribute/child-order changes to matching DOM nodes.
Unchanged nodes remain connected, including document shell and stylesheets.
Input value/checked, textarea text and select selection preserve user edits when
their declarative values are unchanged; an explicit source value change overrides
the live value. The shared control lifecycle captures live state before attributes
or child order change, then restores it by retained DOM reference. It covers
text/textarea drafts, checkbox checked/indeterminate state, radio groups, native
file selections (without assigning FileList/value), single/multiple select and
retained option identity. A changed selected declaration overrides the select's
live selection; removal of a selected option uses native single-select fallback
or the surviving selections of a multiple select. A new checked radio declaration
takes priority over saved group state.
Focus and selection start/end/direction follow the same surviving node, never
the old plan index. Removing the focused item does not focus a different item
that occupies its position. Viewport and nested element/textarea scroll offsets
are restored after focus; browser layout may clamp offsets if content shrinks.
Removed controls/listeners are cleaned up. Components still flatten into ordinary View nodes:
this preserves DOM identity, not component-local cells recreated by a renderer.
Typed `text`, `bind_attrs` and `children` callbacks evaluate only their observed
bindings/regions. Eager reads outside bindings retain full-render compatibility.
See `14-fine-grained-view.md`. Initial hydration still validates the shared plan.

Mismatch behavior is explicit:

- Default: throw `VutHydrationMismatchError` with reason, node index when known,
  expected and actual values; leave existing DOM untouched and do not retain new
  listeners/subscriptions.
- `fallback: "replace"`: for a DOM/plan mismatch only, mount a fresh View into
  the container and report `mounted.hydrated == false`.
- Malformed/unsupported bootstrap, invalid Page signature, and typed data
  marshalling errors remain errors; they do not silently fall back.

Successful attachment owns event callbacks and cell dependencies until refresh,
unmount or host disposal. Refresh suppresses dispatch during the DOM commit and
removes old listeners/callbacks after committing the new tree; unmount cancels
pending animation frames, removes listeners, releases cell
subscriptions and clears the mounted container. Callback removal is safe during
dispatch and cleanup is idempotent. Unique HTML IDs support stable list-node moves.

Virtual DOM, HMR, browser state preservation across server restarts, arbitrary
runtime-reflection-based data serialization, and a template engine are outside
these steps. Eligible nominal Page data uses the compiler-generated typed JSON
schema described above.

## Step 5 — Automatic typed browser registry and hydration build (DONE)

The application opts into browser hydration by calling the typed Page API:

~~~vut
response.render_hydratable(HomePage)
response.render_hydratable(TodoPage, ClientInitial(page: "todo", count: 0))
~~~

A Page is one statically named, non-generic Vut function with signature
`fn() -> View` or `fn(Data) -> View`. The optional argument is one concrete
public nominal `data` value that satisfies `json.Encodable`. The same Page
function builds SSR HTML on the server and the View used by browser hydration.
No browser section in `vutnet.toml`, consumer client `[[bin]]`, client source
file, factory or decoder declaration is required. Normal `response.render`
remains SSR-only.

### Typed registry generation

VPM examines the checked Vut program for calls resolved to
`vutnet.response.render_hydratable`. It rejects an indirect/computed Page
callable, a closure, a variadic/generic Page, an unsupported Page signature or
non-nominal page data with a compile error. Each direct call-site receives the
stable, versioned Vut compiler call-site ID already used by `@track_caller`.
This ID is independent of filesystem root and is identical in server and browser
builds of the same logical source.

For each registration VPM generates a temporary Vut module with a concrete
typed wrapper that invokes the original Page. It also writes a
`vutnet.client-registry` JSON manifest containing the Page ID, typed factory
export, optional decoder name and canonical nominal data identity. These files
live in the VPM hydration staging directory; VPM does not edit the application's
source tree or manifest. The compiler selects the generated entrypoint and
retains each typed wrapper as a JS binding export. It does not replace the Page
type with `dyn`, reflection or raw pointers.

The ordinary data path uses the compiler-generated nominal JSON schema. When
the public Page data type declares a static `from_json`, VPM generates a typed
wrapper around `vutnet.hydration.decode_bootstrap[T]` so custom
`json.Decodable` behavior executes in WASM automatically. The original bootstrap
JSON source is passed to Vut to preserve large integer precision. A custom
`to_json` wire shape should have a matching static `from_json`. Data is public
browser-visible state and must not contain secrets.

### Build and serve lifecycle

The Vutnet CLI remains Vut code in this package's `[[bin]]`; its native Rust
binding orchestrates builds and browser assets. For a dev generation Vutnet
builds the native application, then invokes VPM's
`vpm build --hydration-registry --target wasm32-unknown-unknown` path. VPM
checks the same typed source/dependency graph, compiles the selected generated
entrypoint for WASM, and emits `vutnet-client.wasm`, its generated JavaScript
bindings and `vutnet-client.registry.json`. Native-only VPM link artifacts are
excluded from this browser target.

Vutnet stages those outputs with its bootstrap/navigation scripts in the
generation-scoped private asset directory served at `/__vutnet/`. Build failure
rejects the candidate: the dev supervisor keeps the last-known-good server and
registry/client assets active. Build generations do not overwrite a running
Windows executable. The terminal history is preserved across successful
rebuilds.

`vutnet dev`, `vutnet run` and `vutnet build` build this client automatically.
If no hydratable Page call-sites exist, VPM emits no browser client. A successful
production bundle includes an optional browser directory with the WASM, JS
bindings, registry, bootstrap, navigation and build identity. `vutnet start`
validates and serves that packaged bundle without invoking VPM or reading a
browser-client configuration.

### SSR bootstrap and browser mount

The versioned inert bootstrap now identifies both the typed server data and the
registered Page:

~~~html
<script type="application/json" data-vutnet-bootstrap="v2">…</script>
~~~

~~~json
{
  "protocol": "vutnet.hydration",
  "version": 2,
  "page_id": "v1-0123456789abcdef",
  "has_data": true,
  "data": { "count": 0 },
  "plan": { "version": 2, "nodes": [] }
}
~~~

Vutnet's browser bootstrap loads the generated registry and bindings, resolves
the Page by `page_id`, and calls
`hydrateView(document.documentElement, Page[, options])` on the existing SSR
DOM. For nominal schema decoding the wrapper receives its exact data type. If
the Page has a generated custom decoder, the bootstrap passes that typed decoder
to the existing hydrate API. The Page creates its browser state from the same
initial data, then the shared hydration matcher validates the View plan and
attaches event handlers/subscriptions without replacing the initial DOM.
Navigation uses the same registry lookup for each fetched hydratable response;
unsupported responses retain normal browser navigation.

A malformed/unsupported bootstrap, missing Page ID, registry mismatch,
decoder error or plan mismatch is reported and leaves SSR HTML visible. Cleanup
continues through the existing View mount and callback lifecycle. The loader does
not create a second renderer, callback ABI, state runtime or router.

## Production bundle (DONE)

`vutnet run` builds a debug bundle; `vutnet build` builds a release bundle.
Both build the native app and generated browser registry from the same project
sources. A successful candidate publishes
`build/{debug|release}/vutnet/` with `app[.exe]`, a versioned manifest and,
when hydratable Pages exist,
`browser/{client.wasm,client.js,client.registry.json,bootstrap.js,navigation.js,build-id}`.
A failed client build leaves the previous complete bundle intact. Publication
stages separately before replacement; stop a running bundle before rebuilding
on Windows.

`vutnet start` validates the release manifest and required browser assets and
launches the executable with its packaged browser directory. It does not invoke
VPM, a compiler or a browser-client build. Native serving uses the existing
private `/__vutnet/` mount. Project-owned static directories remain relative to
the project root and must be deployed with the app. Bundles are host/target
specific, not cross-platform executables.

Browser DOM updates continue to use the existing binding and keyed-region
implementation. External stylesheets/classes are supported; native `css` map
declaration serialization and inline `style()` nodes are not implemented by the
browser host. HMR, a template engine and browser live reload on source rebuild
remain outside this phase.
## Step 6 — Same-origin client navigation (DONE)

The hydratable browser bootstrap installs delegated link and submit listeners.
Eligible same-origin GET links and same-origin GET/POST forms request the full SSR
document from the existing server Router; there is no second client Router or
navigation-only server endpoint. Forms honor native constraint validation,
successful controls, submitter values and button overrides. GET fields
replace the action URL's query, matching native form submission. POST supports URL-encoded and multipart bodies; apps should return
303 Post/Redirect/Get for conventional refresh/back behavior. A direct hydratable
POST response can be committed, but refreshing that history URL sends GET. The
browser never silently repeats an unsupported POST.

The browser sets the temporary
`data-vutnet-navigation-state="loading"` attribute on `<html>` and dispatches
document lifecycle events while fetching.
It validates the HTML envelope and versioned client-loader URL, keeps the current
page visible, then disposes the old View mount. It retains the live `Document`,
`<html>`, `<head>` and `<body>` nodes, synchronizes shell attributes and replaces
only head/body children from the SSR response. Matching external stylesheet nodes
are retained by URL/presentation attributes. The generated registry selects the
target Page, and `hydrateView` validates its plan against the committed SSR DOM.

`pushState` records successful navigation. `popstate` fetches the selected URL
through the same server route and hydrates its bootstrap. Scroll positions are
remembered for up to 100 URLs. Same-origin links with `rel="prefetch"` may start
a speculative GET on hover/focus; only hydratable responses with explicit positive
cache freshness are kept, for at most 30 seconds and 20 entries. Hash-only,
modified, external, download and non-self-target links remain native. GET
fallbacks use ordinary document navigation for unsupported HTML, client build
mismatch, scripts, Content Security Policy, attachments or hydration/network
failure. For POST, only a followed redirect can safely fall back to a new GET.
JavaScript-disabled SSR remains ordinary browser navigation.

See `specs/15-client-navigation.md` for the full V1 contract and limits.

## Verification

Focused checks added for this slice:

- Vutnet package source type-check, including the generic typed response API;
- native tests for marker version/occurrence formatting, script-safe JSON, and
  insertion into `<body>`;
- the real VPM/Hyper View fixture for typed and no-data bootstraps, repeated
  `for`-site occurrences, payload escaping, plan structure, and unchanged normal
  SSR output;
- `crates/vpm/tests/vutnet_view_hydration_e2e.rs`: the same shared Page source is
  compiled for a real VPM-built native Vutnet/Hyper server and browser WASM. The
  test fetches the native SSR response over a real socket, then hydrates those
  exact response bytes in headless Chromium. It covers a typed data Page,
  layout/slots, nested components, loop/conditional entries, typed events, state
  refresh, mismatch/fallback, stable initial node identity, invalid data,
  repeated interaction and unmount cleanup;
- root compiler call-site tests for deterministic IDs and native/WASM parity.
- `crates/vpm/tests/vutnet_dev_hydration_e2e.rs`: starts a real Vutnet dev
  supervisor, builds the configured Wasm client and generated bindings, fetches
  real SSR over Hyper, loads those assets in headless Chromium, and verifies
  increment/decrement callbacks update state and DOM without another `/app`
  document request. It also exercises same-origin links, hydration after navigation,
  Back/Forward and full-navigation fallback for SSR-only responses. The
  external-stylesheet, scrolled-page regression checks
  stable document/head/body/link/stylesheet identities, no stylesheet detachment
  or reload, viewport scroll/focus preservation, and reactive title updates.

`cargo test -p vpm --test vutnet_view_hydration_e2e --offline` passes on the
installed headless Edge/Chromium runner, consuming HTML returned by the native
VPM/Hyper fixture rather than a JS-synthesized plan/DOM. Both compiler targets
use the same unprefixed VPM package-root identity (`main`) for this fixture;
different package identities correctly produce different site IDs and a
hydration mismatch. The separate `vutnet_view_http_e2e` also verifies native
SSR bootstrap/plan serialization; compiler tests cover native/WASM call-site
parity. Production uses the same real browser assertions for `run` and
`build/start`, including failed candidate retention and start without a compiler.
Reactive E2E verifies retained text/input/component nodes, user edits, selection,
controlled value changes, list insertion/reorder/deletion, and cleanup.
Custom browser `Decodable` execution and typed client initialization pass the
same dev/run/build/start E2E with initial count 41 and a custom JSON key `start`
that differs from the nominal field `count`. Negative checks cover wrong nominal
decoders, foreign-instance callables, invalid payloads, malformed JSON, exact i64
values above JavaScript's safe range, repeated decoding, duplicate mounts,
idempotent disposal and successful hydration after disposal. Donix demonstrates
the same pipeline with server initial count 7 and JSON key `initial_count`.
Fine-grained text/attribute/children bindings also pass this pipeline. Dedicated
Chromium tests prove unaffected Page/components do not run and removed bindings
release subscriptions. Automatic JSON derivation remains future work.

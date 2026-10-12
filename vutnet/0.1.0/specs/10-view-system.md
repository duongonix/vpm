# Vutnet View System — Design and Phased Plan

Status: the pure-Vut tree builder, slot/layout resolution, HTML5ever-backed SSR,
typed Page-to-Response integration, inline `<style>` rendering, external
stylesheet `<link>` rendering, browser mounting, and typed DOM events for
interactive/form tags are implemented. Browser events marshal owned Vut event
snapshots and keep listener/callback cleanup tied to mount lifetime. State cells
remain a separate Vutnet web API; this change does not redefine their semantics.
Generic Vut call-site identity and opt-in hydratable SSR markers are also
implemented. `response.render_hydratable(Page[, data])` emits a versioned JSON
bootstrap and shared site/occurrence plan; `hydrateView` validates that plan and
attaches the typed Page to existing DOM without replacing it. State updates
after hydration evaluate only subscribed typed binding callbacks and patch their
text, attributes or scoped children. Eager cell reads outside explicit bindings
retain full-render compatibility. Production build/start include browser assets.
See `14-fine-grained-view.md` for the binding contract.
Vut language behavior in root specs and the current compiler/runtime is
authoritative.

## 1. Goals and boundaries

`View` belongs to Vutnet. It does not add Vut keywords, compiler primitives,
HTML syntax, template syntax, component syntax, layout syntax, or state syntax.
The design uses ordinary Vut functions, nominal data, typed function values,
Receiver Functions, closures, `if`/`for`, and the existing ownership model.

The View system targets:

- typed Vut functions returning `View` as components, with ordinary statically
  checked parameters as props;
- an owned tree of elements, text, and fragments;
- explicit, typed construction APIs that work with the current receiver-call
  semantics;
- layout and slot composition resolved before rendering;
- server-side HTML output integrated with Vutnet `Response` through the public
  managed native-import ABI;
- typed browser events on a mounted View, using the existing WASM host callback
  bridge and owned event snapshots rather than exposing DOM objects to Vut.

CSS uses ordinary HTML `<style>` and `<link>` elements, with an optional typed
`css: map[str, str]` shortcut on each HTML element for inline declarations.
There is no stylesheet DSL, CSS-in-JS engine, scoping, or class generation. The
implementation must not add a second template/component language or change the
`Controller(Context) -> Response` contract.

## 2. Findings that constrain the API

### 2.1 Receiver bodies return `void`

A tag/container body is a Receiver Function such as
`fn(ChildrenScope)() -> void`. The compiler does not implicitly turn a final
`str` expression into a child when the expected receiver callback returns
`void`. A minimal compile probe confirmed that a direct string child is a type
error. Therefore this proposal requires explicit child insertion:

~~~vut
view:
  div(attrs = @[]):
    text("Hello")
~~~

The shorthand `div: "Hello"` is unsupported unless a future general Vut
language change defines a conversion; Vutnet must not emulate it with
stringification. A component call is different: when a `void` receiver body
provides a statically matched `child(View) -> void` method, a standalone
View-valued expression such as `UserCard(user)` is implicitly lowered to that
method. This is generic receiver-function behavior, not a View-only compiler
special case; `child(UserCard(user))` remains valid.

### 2.2 `View` is the type; `view(...)` builds it

The public nominal type is `View`. Its ordinary builder is the lowercase
function `view(...)`; public examples must not call `View(...)` as a View DSL
factory. Existing internal `View(_nodes: ...)` expressions are the compiler's
nominal data constructor and remain private implementation details.

### 2.3 Callable transport and closure constraints

Vut ordinary closures can capture values and are deterministically retained and
dropped in Vut code. That alone does not make a closure promotable to the public
Native Callback ABI. ABI v1 supports typed callback fields and invocation for
named/non-capturing callables; it rejects capturing environments. Managed
native-import v1 supports lists and nominal data but does not generally admit
callables in its ordinary value graph. Any native renderer boundary must use
the public ABI path and SDK that supports the relevant callable graph, or keep
event callbacks out of that crossing. No private Runtime Handle ABI, raw
callable address, `dyn`, or Vutnet-specific transport is allowed.

### 2.4 Module, receiver, and default-argument constraints

The current compiler requires a Receiver Function method's receiver type to be
declared in the same module. Data fields are module-private. It also requires
required parameters to precede parameters with defaults. Tag configuration
parameters, including `attrs = @[]`, have defaults; a non-void tag's required
receiver callback follows those options as the final signature parameter.
Receiver methods are resolved implicitly inside the typed receiver body and
are not imported as ordinary module functions. The verified application import
is:

~~~vut
import vutnet.view at View, view, attr, bool_attr
~~~

A layout is a normal typed Vut function returning `View`. A Page can pass a
factory by name (`view(layout = SiteLayout):`) or pass the result of a typed
factory call (`view(layout = DashboardLayout(user)):`). Both paths use typed
callable/value overloads; there is no optional callable sentinel. Layout
functions use the same public `view()` builder as Pages. While a layout is
composed, its slot declarations are interpreted as placeholders by the builder;
that bookkeeping is private and is not represented by a separate public
template API.

The initial builder keeps `View`, `Attribute`, `ViewNode`, `ChildrenScope`,
their receiver methods, and private-field access in `src/view/mod.vut`. This is
a constraint-driven cohesive module. Splitting later requires public typed
accessors or compiler support for cross-module receiver methods; do not expose
node internals as a shortcut.

## 3. Public Vut API proposal

The public module is `vutnet.view`. The import syntax is verified by a focused
compiler/runtime fixture:

~~~vut
import vutnet.view at View, view, attr, bool_attr
~~~

The import spelling passed compiler/runtime fixtures mounted under the
`vutnet.view` package namespace, typed response compile checks, and real-socket
VPM integration. A component is any ordinary typed Vut function returning
`View`; no separate `Component` keyword or required nominal component type is
introduced.

~~~vut
type _Layout = fn() -> View
fn view(body: fn(ChildrenScope)() -> void) -> View
fn view(layout: _Layout, body: fn(ChildrenScope)() -> void) -> View
fn view(layout: View, body: fn(ChildrenScope)() -> void) -> View
fn ChildrenScope.slot(name: str) -> void
fn ChildrenScope.slot(name: str, body: fn(ChildrenScope)() -> void) -> void
fn attr(name: str, value: str) -> Attribute
fn bool_attr(name: str, enabled: bool) -> Attribute
fn ChildrenScope.text(value: str) -> void
fn ChildrenScope.child(value: View) -> void
fn View.root_count() -> int
fn View.text_content() -> str
fn View.element_id_count(id: str) -> int
~~~

`text` and `child` are receiver methods, called implicitly inside `view:` or
a tag body and not imported as ordinary functions. `root_count()` returns the
number of roots; `text_content()` concatenates text nodes in document order.
These read-only methods keep node representation private.

`ViewNode` is implementation-owned. Its conceptual variants are `Element`,
`Text`, and `Fragment`; it is not a public API commitment. A `View` always owns
its node tree and has value semantics. `child(view)` inserts component roots as
a fragment, preserving order and allowing multiple roots.

HTML tag functions are ordinary typed receiver functions. Each tag retains the
custom-attribute escape hatch `attrs: list[Attribute] = @[]`; existing calls
such as `attrs = @[attr(...)]` remain the API for one or more custom
attributes. Every HTML element also accepts `css: map[str, str] = ()`; the
empty map default is inferred from the parameter type. CSS declarations become
the element's single `style` attribute through the existing Attribute/native
validation/HTML5ever serialization path. Per-tag shortcut parameters are
optional and typed, then any non-void tag ends with its required void receiver
body. Required parameters precede optional parameters to follow Vut's signature
rules. Shortcuts, custom attributes, and CSS declarations share one
`list[Attribute]` representation and one native validation/serialization path;
CSS entries carry private kind metadata so raw `attr("style", ...)` remains
rejected. The new `css` option follows the existing `attrs` option in each
signature, preserving the positional slot of `attrs` for existing callers.

~~~vut
fn ChildrenScope.div(
  id: str? = null,
  class: str? = null,
  attrs: list[Attribute] = @[],
  css: map[str, str] = (),
  body: fn(ChildrenScope)() -> void
) -> void

fn ChildrenScope.h1(
  id: str? = null,
  class: str? = null,
  attrs: list[Attribute] = @[],
  css: map[str, str] = (),
  body: fn(ChildrenScope)() -> void
) -> void
~~~

The implementation has richer per-tag signatures. Its available shortcuts are:

| Tags | Shortcuts |
| --- | --- |
| `html` | `id`, `class`, `lang` |
| `head`, `body`, `div`, `span`, `header`, `footer`, `main`, `section`, `article`, `aside`, `nav` | `id`, `class` |
| `h1`–`h6`, `p`, `strong`, `em`, `small`, `pre`, `code` | `id`, `class` |
| `blockquote` | `id`, `class`, `cite` |
| `a` | `id`, `class`, `href`, `target`, `rel`, `download` |
| `img` | `id`, `class`, `src`, `alt`, `width`, `height`, `loading` |
| `ul` | `id`, `class` |
| `ol` | `id`, `class`, `start`, `reversed` |
| `li` | `id`, `class`, `value` |
| `button` | `id`, `class`, `type` (`button_type`), `name`, `value`, `disabled` |
| `input` | `id`, `class`, `type` (`input_type`), `name`, `value`, `placeholder`, `required`, `disabled`, `checked`, `readonly`, `autocomplete`, `min`, `max`, `step`, `minlength`, `maxlength` |
| `textarea` | `id`, `class`, `name`, `placeholder`, `rows`, `cols`, `required`, `disabled`, `readonly`, `minlength`, `maxlength` |
| `form` | `id`, `class`, `action`, `method`, `enctype`, `autocomplete`, `novalidate` |
| `label` | `id`, `class`, `for_id` (HTML `for`) |
| `select` | `id`, `class`, `name`, `required`, `disabled`, `multiple` |
| `option` | `id`, `class`, `value`, `selected`, `disabled` |
| `table`, `caption`, `thead`, `tbody`, `tfoot`, `tr` | `id`, `class` |
| `th` | `id`, `class`, `colspan`, `rowspan`, `scope` |
| `td` | `id`, `class`, `colspan`, `rowspan`, `headers` |
| `audio` | `id`, `class`, `src`, `controls`, `autoplay`, `loop`, `muted`, `preload` |
| `video` | `id`, `class`, `src`, `controls`, `autoplay`, `loop`, `muted`, `preload`, `poster`, `width`, `height` |
| `source` | `src`, `type` (`source_type`), `media` |
| `iframe` | `id`, `class`, `src`, `name`, `width`, `height`, `loading`, `allow`, `sandbox` |
| `title` | `id`, `class` |
| `meta` | `name`, `content`, `charset`, `http_equiv` (HTML `http-equiv`) |
| `link` | `id`, `class`, `href`, `rel`, `type` (`link_type`), `media` |
| `style` | `id`, `class`, `media` |
| `script` | `id`, `class`, `src`, `type` (`script_type`), `async_enabled` (HTML `async`), `defer` |

Boolean shortcuts use `bool`; true emits an HTML boolean attribute and false
omits it. The same rule applies to `bool_attr`. Integer values use `int?` for
`width`, `height`, `rows`, `cols`, `start`, `li.value`, `rowspan`, `colspan`,
`minlength`, and `maxlength`. `min`, `max`, and `step` on `input` use `float?`.
Button, input, and option `value` shortcuts use `str?`; remaining shortcuts
also use `str?`. The boolean set is `disabled`, `required`, `checked`,
`readonly`, `multiple`, `selected`, `controls`, `autoplay`, `loop`, `muted`,
`reversed`, `novalidate`, `async_enabled`, and `defer`.

`attr(name, value)` takes positional arguments and can represent custom names,
including `data-*` and `aria-*`; arrays keep the established multi-attribute
syntax. `css` is a typed map from CSS property name to one CSS value. Every
element tag accepts it, and the native boundary parses each name/value as one
CSS declaration. A top-level semicolon cannot inject another declaration. The
parser checks CSS syntax; it does not guarantee that a browser recognizes a
property or supports its value. Unknown but syntactically valid properties are
left for the browser to ignore. Do not pass untrusted values as CSS; parsing is
not CSS sanitization.

Vut maps do not guarantee iteration order. Vutnet sorts CSS property names
lexicographically before serialization, making output deterministic. When
shorthand and longhand properties overlap, that sorted order determines which
declaration wins in the CSS cascade; avoid conflicting declarations when that
order is not the desired one. All generated declarations are combined into a
single `style` attribute. `attr("style", value)` and CSS entries in `attrs` stay
rejected so callers use the typed `css` map. HTML5ever escapes the final
attribute value. Event attributes remain rejected; the `<style>` element is a
separate supported tag. Tag argument completion follows each resolved function
declaration, so `css` is offered for every tag.
Illustrative component API:

~~~vut
import vutnet.view at View, view, attr, bool_attr

fn UserCard(name: str, age: int) -> View:
  view:
    div(class = "card", attrs = @[attr("data-component", "user-card")]):
      h2:
        text(name)
      p:
        text("Age: $age")

fn Home(visible: bool) -> View:
  view:
    div(id = "profile", class = "app", attrs = @[bool_attr("hidden", !visible)]):
      h1:
        text("Hello")
      UserCard(name = "Donix", age = 20)
~~~

Named function arguments use `=`. Props are ordinary function parameters and
retain normal compiler type checking. Vut `if`, `for`, `match`, and local
variables are used directly inside receiver bodies; no template control-flow
directives are defined. Text still requires `text(...)`; tags use their receiver
functions. A standalone expression returning `View` is inserted through the
receiver's typed `child(View)` method, so components can be called directly:

~~~vut
view:
  div:
    HelloCard(name = "Vut")
~~~

The compiler statically resolves this as `child(HelloCard(name = "Vut"))` only
when the active receiver has a unique `child(View) -> void` method. Other
expression statements retain their ordinary Vut semantics.

## 4. Tree, attributes, and HTML safety

The owned tree has these semantics, independent of physical representation:

~~~text
View
└── ordered roots
    ├── Element(tag, attributes, events, children)
    ├── Text(str)
    └── Fragment(ordered children)
~~~

Phase 1 uses a flat preorder node list with a text/element kind flag and parent
index (`-1` denotes a root); root sequences and inserted component roots provide
Fragment semantics without a public Fragment constructor. An element builds
children in a separate local scope, then appends its node and children with
remapped parent indices. This avoids losing nested mutations when Vut's COW
lists cross receiver calls. `child(view)` copies nodes and remaps component
roots to the current parent, preserving order and multiple roots. The tree
stores no borrowed references to locals, request buffers, or Rust stack values.

The Phase 1 attribute API is deliberately typed:

- `attr(name: str, value: str) -> Attribute` for text values;
- `bool_attr(name: str, enabled: bool) -> Attribute` for HTML boolean
  attributes; disabled attributes are omitted by SSR;
- no implicit conversion of arbitrary data, numbers, or `bytes` to text;
- event bindings use a separate typed representation and are never emitted as
  `on*` HTML attributes;
- do not stringify values implicitly.

Tag shortcuts are statically checked according to the tag-specific signatures in
Section 3. Regular shortcuts are normalized into `Attribute(name, value,
is_boolean, enabled)` and concatenated with the supplied `attrs` list. CSS map
entries use the same private Attribute representation with a CSS-kind marker;
the native validator combines them into one HTML `style` attribute. Because Vut
lists use value/COW semantics, helper functions return their updated list and
callers reassign it; helpers must not silently mutate a discarded copy.
Rendering validates tags and attributes: duplicate HTML names are rejected
case-insensitively; names must use the supported HTML attribute-name form; raw
`style` and `on*` attributes are rejected; CSS property names and values are
checked as single declarations; URL-valued attributes reject controls/whitespace
and accept only `http`, `https`, `mailto`, `tel`, or relative URLs. The supported
tag set is the builder set in Section 3.
Rendering must escape text and attribute values, preserve child order, handle
fragments and HTML void elements, and reject malformed tags/attributes. URL
attributes such as `href` and `src` need an explicit safe-scheme policy before
rendering; they must not be treated as trusted merely because they are typed
strings. Event callbacks must never be serialized as inline JavaScript.

Phase 3 uses `html5ever` 0.27 and `markup5ever` 0.12's HTML serializer. A small
Vutnet-owned intermediate tree is built from the validated flat View nodes; the
serializer owns text/attribute escaping and HTML void-element output. Vutnet
owns schema/parent-index checks, tag/attribute allowlists, duplicate checks and
URL policy. Serialization consumes a fully resolved tree and has no layout or
slot semantics.

## 5. Layouts and slots

Layouts remain ordinary functions returning `View`; no inheritance or template
syntax is introduced. Slots are Vutnet data resolved by a Vutnet composition
step before the tree reaches the renderer. The conceptual resolution is:

~~~text
page view + layout view + named slot supplies
                 ↓ resolve
owned element/text/fragment tree with no slot nodes
                 ↓ validate and render
HTML
~~~

A slot declaration has a name and is either required or optional with fallback
content. A page supplies ordered content by name. Resolution rules:

1. a supplied value replaces fallback content;
2. absent optional slots use their fallback;
3. an absent required slot returns a render error;
4. duplicate declarations or duplicate supplies return a render error;
5. an unknown supplied name returns a render error and may include a suggestion
   when a close declared name exists;
6. successful output contains no slot placeholder nodes.

The public layout and slot API uses ordinary View values and receiver functions:

~~~vut
fn SiteLayout() -> View:
  view:
    html(lang = "en"):
      head:
        meta(charset = "utf-8")
        slot(name = "head"):
          title:
            text("Site")
      body:
        main:
          slot(name = "content"):
            p:
              text("Fallback content")

fn DocsPage(data: DocsData) -> View:
  view(layout = SiteLayout):
    slot(name = "head"):
      title:
        text(data.title)
      meta(name = "description", content = data.summary)
    slot(name = "content"):
      h1:
        text(data.title)
      p:
        text(data.summary)
~~~

`slot(name = "header")` declares a required slot. Supplying a receiver body
declares an optional slot and provides its fallback; there is no `required`
flag. At the top level of a Page's `view(layout = SiteLayout):` body, slots
capture supplied content. Ordinary roots outside named slots are supplied as
the conventional `content` slot; combining ordinary roots with an explicit
`slot(name = "content")` creates duplicate supplies and is a render error.
Document metadata should be supplied through a named `head` slot declared
inside the layout's `head` element, while page body content uses `content`.
Names are exact and case-sensitive. Duplicate
declarations or supplies, unknown supplies, empty declaration names, and a
missing required slot produce a typed low-level render error. Successful
composition removes all placeholders before rendering. Layouts with typed
arguments are called normally, for example `view(layout = DashboardLayout(user)):`.

`_SlotSupply`, error variants, and the slot resolver are module-private. There
is no public `view_template`, `fill`, or `resolve` API. Supplied Views must
already be resolved; compose from the innermost layout outward. Calling
`view()` without a layout produces a standalone View; declaring slots without
a layout records an error surfaced by the low-level renderer. Failures from a
nested component's layout propagate through receiver scopes. High-level
`response.render(...)` maps these failures to HTTP 500.

Names are runtime-checked because Vut cannot statically validate an arbitrary
string against a runtime-selected layout without reflection. No reflection or
compiler metadata is introduced.

~~~vut
import vutnet.view at View, view

fn SiteLayout() -> View:
  view:
    main:
      slot(name = "header")
      slot(name = "content"):
        p:
          text("Choose a section")

fn DocsPage(page_data: DocsData) -> View:
  view(layout = SiteLayout):
    slot(name = "header"):
      h1:
        text(page_data.title)
    slot(name = "content"):
      p:
        text(page_data.summary)
~~~

Define supplied content as a component function returning `View` when a
receiver block is needed as a named argument.

### 5.1 Inline CSS and external stylesheets

CSS uses the existing View tree and HTML serializer:

- `style(...)` creates an ordinary `<style>` element whose children must be
  `text(...)` nodes containing CSS text;
- `link(...)` creates the existing void `<link>` element for an external file;
- each HTML element accepts `css: map[str, str] = ()` for typed inline
  declarations;
- there is no stylesheet DSL, bundler, scoping, class generation, or component
  style hoisting.

`style()` accepts `id`, `class`, `media`, `css`, and `attrs`. `style` is a
non-void element, so its receiver body is the normal final parameter. `attrs`
uses the same `attrs = @[attr(name, value)]` path as other tags. Inline element
styles use the `css` map; raw HTML style attributes remain rejected. The
current Vut API imports and calls ordinary functions without a CSS-specific
import convention:

~~~vut
import vutnet.view at View, view, attr

fn SiteLayout() -> View:
  view:
    html(lang = "en"):
      head:
        slot(name = "head")
      body:
        slot(name = "content")

fn DocsPage() -> View:
  view(layout = SiteLayout):
    slot(name = "head"):
      title:
        text("Donix")
      meta(name = "description", content = "Donix app")
      link(
        href = "/styles/app.css",
        rel = "stylesheet",
        link_type = "text/css"
      )
      style(id = "page-css", media = "(min-width: 768px)"):
        text("""
          .app > .title {
            max-width: 1200px;
          }
        """)
    slot(name = "content"):
      div(
        class = "app",
        css = (
          "display": "none",
          "border": "1px solid blue"
        )
      ):
        h1(class = "title"):
          text("Welcome to Donix")
~~~

Vut supports triple-quoted multiline strings; their indentation and
interpolation follow the root language specification. CSS text inside a
`<style>` element is emitted as HTML raw text, preserving `<`, `>`, `&`, quotes,
Unicode, and newlines without HTML text escaping. It is not an arbitrary
raw-HTML API: a `<style>` element with any Element child fails rendering. To
prevent breaking out of the raw-text element, a case-insensitive `</style`
sequence is rejected across the combined text children and becomes a render
error (and HTTP 500 through `response.render`). `css` map entries instead become
an escaped HTML `style` attribute and each property/value must parse as one
declaration. The CSS parser checks syntax, not property-specific semantic support
or sanitization; do not interpolate untrusted values into CSS.

`link()` is a void element and accepts no child callback. Its shortcuts are
`id`, `class`, `href`, `rel`, `link_type` (serialized as `type`), `media`, the
usual `css` map, and `attrs`. It is rendered by the same HTML5ever serializer as
all other elements. Attribute ordering is deterministic from the tag's declared
shortcut order, with the combined CSS `style` attribute at the `css` parameter
position, followed by custom attrs; HTML consumers should rely on element and
attribute semantics, not formatting.

For a stylesheet file, use `link(rel = "stylesheet", href = ...)`. CSS
`@import` text is also passed through unchanged; View does not parse it. For
primary page styles, prefer a stylesheet link over CSS `@import`. No Vut
`import` behavior changes: `import "./app.css"` is not supported and CSS is not
a Vut module. Tailwind class names work as normal `class` strings, while its
build/output pipeline remains separate. This CSS serializer is currently native
SSR: the browser host does not yet implement `css` map declaration serialization
or `style()` elements. Hydratable applications should use external stylesheets
and ordinary class strings until that parity work is complete.

The normal `head` slot preserves page node order, so `title`, `meta`, `link`,
and `style` render in the same order supplied by the Page. Component-scoped CSS,
`scoped = true`, selector rewriting, hoisting, and deduplication are out of
scope.

## 6. Rendering and Response integration

Rendering belongs to `vutnet.view`, not the Vut core. The low-level renderer
and high-level typed Response API are:

~~~vut
fn render(value: View) -> result[str, str]
fn render_hydratable(value: View) -> result[str, str]
fn response.html(code: int, content: str) -> Response
fn response.render(page: fn() -> View) -> Response
fn response.render[T](page: fn(T) -> View, page_data: T) -> Response
fn response.render_hydratable(page: fn() -> View) -> Response
fn response.render_hydratable[T: json.Encodable](page: fn(T) -> View, page_data: T) -> Response
~~~

`render` is a managed native import over the public Native SDK. Validation or
serialization errors are returned as `result[str, str]`; a managed ABI/schema
failure remains an ABI error. `response.html` remains a low-level constructor
for already rendered HTML. `response.render` is the standard typed Page
integration: the `fn() -> View` and generic `fn(T) -> View` signatures are
statically resolved overloads. It invokes the Page, delegates layout/slot
composition and serialization to the existing `views.render`, then returns
status 200 with `text/html; charset=utf-8`. Render or layout errors become a
generic HTTP 500 Response; the detailed reason is kept in private diagnostic
metadata for the native request-error log and is never sent to the client. Page
data remains statically typed; a mismatched Page/data pair fails compilation.
For hydratable responses, the data overload retains the `json.Encodable` type
bound, but ordinary eligible `data` declarations receive a compiler-generated
implementation; users declare and construct their typed page-data value without
writing codec methods. The response contains an inert JSON bootstrap and the
shared flattened plan. Browser `hydrateView` marshals the decoded object using
the Page's compiled nominal parameter schema. Custom `json.Decodable` code is
not needed for the default field-matching wire shape; a configured decoder is an
advanced option for custom formats.

~~~vut
import vutnet.response as response
import vutnet.view at View, view

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

This flow passes through a real VPM-built Vutnet/native package and HTTP server
in `crates/vpm/tests/vutnet_view_http_e2e.rs`. The E2E covers typed and no-data
Page callables, layout + slots, iteration over typed page data, response status
and Content-Type, repeated requests, and render failures mapped to HTTP 500.
Do not reserve `vut.render` as the API: Vutnet owns View and rendering. This is
an SSR tree renderer, not a template engine. Render failures never fall back to
unescaped output.

The native import consumes nominal `View`, then reads
`list[ViewNode]`, each node's `list[Attribute]`, and their typed primitive/string
fields using `vut-native-sdk`. It returns the public managed `result[str, str]`.
The SDK owns the borrowed input for the call and result tokens; Vut receives an
owned result and releases it through normal managed-value semantics. The
renderer uses canonical package-scoped nominal IDs and never inspects Vut heap
layout, private Runtime Handles, or raw pointer tokens. The actual package
boundary and HTTP fixture proves this path; no ABI blocker or core change was
needed.

SSR continues to receive the callback-free `ViewRender` projection. Event
handlers remain in Vut-owned `ViewNode._events`, are omitted from HTML output,
and never cross the native renderer boundary. The browser renderer reads the
same Vut `View` tree and attaches listeners separately.

`views.render_hydratable(value)` uses that same projection and serializer but
adds versioned source markers: elements get
`data-vut-site="v2-<16-hex-site-id>-<occurrence>"` and ordinary text nodes get a
preceding `<!--vut-site:v2-<16-hex-site-id>-<occurrence>-->` comment. The plan is
computed after component and layout/slot flattening. It stores the static
compiler site ID and a per-site 0-based runtime occurrence, parent, node kind,
and tag. Text inside `script`, `style`, `title`, and `textarea` keeps its entry in
the plan but has no comment marker because HTML treats comment syntax there as
content. User attributes named `data-vut-site` are rejected in hydratable mode.
The ordinary `views.render` and `response.render` outputs remain unmarked.

The shared plan distinguishes repeated static sites in the initial evaluated
tree. Update identity uses unique HTML IDs for stable list/component roots and
site plus local sibling occurrence under retained parents for other nodes.
Without IDs repeated siblings remain positional. Browser hydration validates
the complete initial tree before attaching events/state. See
[`13-ssr-hydration.md`](13-ssr-hydration.md) and root
`specs/compiler/call-site-identity.md`.

## 7. Events and state/reactivity

### 7.1 Events

Mounted Views accept optional typed callback parameters directly on the
supported tag functions. Callback parameters are statically checked and stored
in the element's `EventHandlers`; they are never converted to HTML attributes.
The public event types live in `vutnet.view.event_types`:

~~~vut
data EventTarget:
  id: str
  tag: str
  name: str

data Event:
  target: EventTarget

data InputEvent:
  _base: Event
  target: EventTarget
  value: str
  checked: bool

data KeyboardEvent:
  _base: Event
  target: EventTarget
  key: str
  code: str
  repeat: bool

fn Event.prevent_default() -> void
fn Event.stop_propagation() -> void
~~~

Every typed event is an owned Vut snapshot. `Event` also provides the target
metadata above; `MouseEvent` adds `button` and modifier flags, `InputEvent` and
`ChangeEvent` add `value` and `checked`, and `KeyboardEvent` adds key data.
`FocusEvent` and `SubmitEvent` carry the common event/target snapshot. No raw JS
Event, DOM object, or borrowed pointer is exposed. Specialized event data shares
the base event control token; `prevent_default()` and `stop_propagation()` act on
the original DOM event only while its Vut callback is executing synchronously.
`event.target` snapshots the originating DOM target, including when a capture
listener such as `form.on_invalid` runs on an ancestor; listener/current-target
objects are not exposed.

| Tag | Typed callback parameters |
| --- | --- |
| `button` | `on_click: MouseEventHandler`, `on_focus` / `on_blur: FocusEventHandler`, `on_key_down` / `on_key_up: KeyboardEventHandler` |
| `input` | `on_input: InputEventHandler`, `on_change: ChangeEventHandler`, `on_focus` / `on_blur`, `on_key_down` / `on_key_up`, `on_click`, `on_invalid` |
| `textarea` | `on_input`, `on_change`, `on_focus`, `on_blur`, `on_key_down`, `on_key_up`, `on_select`, `on_invalid` |
| `select` | `on_change`, `on_input`, `on_focus`, `on_blur`, `on_invalid` |
| `option` | `on_click`; selection changes are handled by `select.on_change` |
| `form` | `on_submit: SubmitEventHandler`, `on_reset`, `on_invalid` |
| `label` | `on_click: MouseEventHandler` |

The shared callback aliases use the matching event type and return `void`.
`on_invalid` uses a capture listener because native invalid events do not bubble.
`select.on_change` receives the current selected value through `event.value`.
Other tags do not accept event parameters unless listed in their receiver
signature. LSP named-argument completion reads those signatures, and completion
inside `fn(event):` uses the callback's inferred nominal event type.

~~~vut
import vutnet.view at View, view
import vutnet.view.event_types at InputEvent, SubmitEvent

fn Save(event: InputEvent) -> void:
  out(event.value)

fn Login(event: SubmitEvent) -> void:
  event.prevent_default()

fn Page() -> View:
  view:
    input(input_type = "text", on_input = Save)
    form(on_submit = Login):
      button(button_type = "submit"):
        text("Save")
~~~

The browser bridge maps DOM events to only the fields in the declared Vut event
schema. `value`, `checked`, key values, and target metadata are copied before the
callback runs. It associates a short-lived opaque control ID with the original
DOM event so `prevent_default()` and `stop_propagation()` can act without
marshalling the JavaScript object into WASM memory. Calls after callback return
fail as an inactive event operation.

Mount attaches one listener for each non-null handler. `refresh()` suppresses
dispatch during the DOM commit, then detaches old listeners and releases their
callback wrappers after committing the new tree;
after an event, one refresh is scheduled for the next animation frame so the
browser can finish the related focus/pointer/keyboard interaction first.
`dispose()` removes listeners,
clears the mounted DOM, and releases callbacks after any synchronous in-flight
call completes. Repeated refresh does not accumulate listeners. SSR reads a
callback-free projection and emits only attributes and children.

The older `vutnet.view.events.InteractiveView` click dispatcher remains an
in-process compatibility/testing API. Browser mounting uses `View`'s typed event
parameters and does not serialize either API as `onclick="..."`. Named and
non-capturing callbacks are covered by browser E2E; capturing callback
environments remain constrained by the current WASM callback/runtime support
and are not promised as a cross-host capability.

### 7.2 State

`vutnet.web.state(value)` returns the core identity-bearing `cell[T]` handle.
The module provides typed `state_get`, `state_set`, `state_update`, and
`state_revision` helpers. `state_update` is a read, callback, and write sequence
rather than an atomic read-modify-write operation.

Browser `mountView` tracks reads separately for typed text/attribute/children
bindings and the compatibility root renderer. Writes invalidate only subscribers;
multiple writes coalesce on the next animation frame. Regions run before their
descendants so removed bindings cancel pending work. Successful evaluations replace
their dependencies; failures preserve the previous DOM/subscription. Unmount or
host shutdown cancels work and releases all subscription-owned cell handles.
SSR remains a one-shot render with no browser subscriptions.
For server-derived initial values, the VPM-generated Page wrapper invokes the
same typed Page with data from the versioned bootstrap. Page code creates browser
cells from those values; subscriptions attach during `hydrateView`. If the
nominal data type has a public static `from_json`, VPM includes a typed decoder
wrapper automatically. See `13-ssr-hydration.md`; eligible page-data records
receive compiler-generated `json.Encodable`.

Typed text and attribute callbacks update their existing nodes; `children`
reevaluates only its scoped child region. Eager reads outside bindings reevaluate
the mounted View using the existing compatibility patcher. Unique HTML IDs are stable
keys; other nodes match by site/local sibling occurrence under matched parents.
Unchanged input values/checked, textarea and select selection preserve user
edits; changed declarative values override them. The shared control lifecycle
captures state before DOM mutation and restores retained input drafts, checkbox
indeterminate/checked, radio groups, file selections and single/multiple selects.
Focus/selection start/end/direction use node references, not plan positions;
nested element/textarea scroll offsets persist subject to layout clamping.
Stable unique IDs on component/list roots preserve their descendants through
insert/delete/reorder. Duplicate IDs reject a candidate before DOM changes;
removed items release listeners and later reinsertion creates fresh nodes.
Components still
flatten into View nodes; cells recreated inside a renderer are not preserved as
component instances. No separate Virtual DOM or state runtime is introduced.
Typed bindings are specified in `14-fine-grained-view.md`; core cell semantics
remain in root `specs/roadmap/reactive-state.md`.

## 8. Package/module structure

The pure-Vut receiver methods and private data remain together because the
compiler requires receiver extensions to be declared beside their receiver:

~~~text
vpm-package/vutnet/src/view/
├── mod.vut            tree, attrs, slots, resolution, tag receivers and render
├── event_types.vut    owned event snapshots, callback aliases and event controls
└── tracking.vut       isolated initial binding evaluation
~~~

`mod.vut` is a cohesive compiler-constrained module, not an endorsement of
monolithic implementation files. Extract only responsibilities that can cross
module boundaries without exposing node internals or changing Vut semantics.
Create only files needed by the active phase; do not add empty state or style
modules.

The native implementation belongs in the existing Vutnet native crate:

~~~text
vpm-package/vutnet/native/src/view/
├── mod.rs
├── parse.rs           typed SDK reads and parent/tree validation
├── validate.rs        tag/attribute/URL policy
└── serialize.rs       HTML5ever serializer adapter
~~~

Add only modules required by the chosen crate and phase. The internal
`native/server` Cargo crate remains an HTTP transport engine and does not gain
View-tree or component semantics.

Standalone VPM examples live outside the publishable package under
`vpm-package/vutnet-examples/`; integration fixtures remain in the existing
VPM/compiler test locations. Tests must use VPM's package build/link path for
package-boundary behavior.

## 9. Phased implementation plan

### Phase 0 — Audit and design (DONE)

- compare Vutnet goals to current Vut receiver, callable, ownership, ABI, and
  module behavior;
- record the direct-text conversion deviation;
- identify public ABI gates for a nested tree;
- keep this phase spec-only apart from minimal syntax probes.

Exit: API and limitations are documented; no compiler/runtime change is
assumed.

Sections 2.1–2.4 record the audit results and compile probes. No Vut core,
compiler, runtime, stdlib, VPM, or native source was changed for Phase 0/1.

### Phase 1 — Pure Vut tree and builder (CORE IMPLEMENTED)

- add `View`, ordered element/text/fragment nodes, `view()`, `text()`, `child()`,
  typed text/boolean attributes, and the initial HTML tag receiver functions;
- use ordinary `fn(...) -> View` components and props;
- verify if/else, loops, multiple roots, nested components, and deterministic
  value ownership using existing Vut semantics;
- use explicit `text(...)`; do not add implicit string-child conversion;
- add focused positive and compile-fail tests for child types, props, attributes,
  and unsupported APIs.

Exit: a real VPM consumer can construct and inspect/use a typed tree without
native rendering.

Implemented in `src/view/mod.vut`: `View`, `Attribute`, `view`, `attr`,
`bool_attr`, receiver methods `text`/`child`, `root_count`, `text_content`,
block tags (`div`, `span`, `p`, `h1`–`h6`, `a`, `button`, `ul`, `ol`, `li`,
`main`, `header`, `footer`, `section`, `article`, `nav`, `strong`, `em`,
`label`) and void tags (`br`, `img`). A focused compiler/runtime fixture passes
through the `vutnet.view` package namespace and covers nested elements, typed
component props, multiple component roots, `if`/`else`, `for`, attributes, and
text order. A compile-fail check rejects an `int` supplied to a `str` prop.
The Vutnet `vpm test` runner could not launch its package-local executable in
this Windows workspace (OS error 5); rerun that package gate in a working VPM
test environment before marking the phase fully verified.

### Phase 2 — Layout and slot resolution (IMPLEMENTED)

- add typed layout composition and named slot supply/fallback/required behavior;
- return typed errors for missing required, duplicate, and unknown slots;
- verify nested layouts and deterministic slot order;
- keep unknown-name checking at runtime unless a future Vut capability enables
  a sound static check.

Exit: resolved trees contain no slot nodes and failure cases are explicit.

Implemented in `src/view/mod.vut`: public `ChildrenScope.slot` overloads use a
body-less call for required slots and a receiver body for optional slots with
fallback content. Slot supplies, typed internal errors, and the resolver are
private. Resolution is deterministic and innermost-to-outermost; successful
views contain no slot markers for the resolved layer. The focused
compiler/runtime test covers supplied content, fallback content,
required/unknown/duplicate supplies, and nested composition through only the
public Page API.

### Phase 3 — SSR ABI proof, HTML renderer, and Response integration (DONE)

- first prove the exact nested Vut data/list schema crosses through the public
  SDK and ABI with correct ownership, without event callbacks;
- select and pin a maintained Rust DOM/HTML serialization crate after behavior
  and output-stability checks;
- validate tree/tag/attribute/URL policy, build the serializer DOM, and return
  escaped HTML as an owned managed string/result;
- add `response.html` with `text/html; charset=utf-8` while preserving the
  existing immutable Response model;
- test real Vut values through the VPM native package boundary and real Vutnet
  controller response.

Exit: actual Vut components render through the native renderer and can be sent
as an HTTP response; ABI/SDK ownership is fully released on success and error.
If the public ABI cannot carry the tree, stop and report the core ABI blocker;
do not bypass it.

The public ABI proof and renderer are implemented without Vut core changes.
`native/src/view/` reads the flat nominal/list schema with `vut-native-sdk`,
validates tree shape and security policy, and serializes via HTML5ever. The Vut
API exposes `views.render(View) -> result[str, str]` as the low-level renderer
and overloaded `response.render(Page) -> Response` and
`response.render(Page, typed_data) -> Response` as typed HTTP integrations.
Real-socket evidence in `crates/vpm/tests/vutnet_view_http_e2e.rs` covers data/no-data Page
callables, layout + slots, typed iteration, `Content-Type`, render failure to
HTTP 500, and real Hyper to HTTP client delivery. The Phase 2/3 focused checks
pass in this workspace.

### Phase 4 — Event tree and callback ownership foundation (DONE)

- choose typed event kinds and signatures;
- represent handlers separately from HTML attributes;
- prove retain/release/drop for supported named non-capturing handlers;
- explicitly reject unsupported capturing closure promotion;
- keep SSR serialization independent from event execution.

Exit: event values are safely represented and lifecycle-tested. This is not a
browser interaction claim.

Implemented in `src/view/events.vut`: one normalized, owned `ClickEvent`
snapshot shape; the statically typed `ClickHandler = fn(ClickEvent) -> void`;
separate `EventBinding` and `InteractiveView` data; unique-ID validation;
in-process typed dispatch; and read-only event accessors. A compiler/runtime
fixture mounted under the `vutnet.view` package namespace repeats a named-handler
dispatch, rejects missing and duplicate IDs/bindings, invokes a Vut-owned
capturing closure, and verifies its closure environment count returns to zero
after scope teardown. A compile-fail test rejects a handler with the wrong
signature. The general callback ABI's named/non-capturing promotion and capture
rejection remain covered by the root FFI tests. Nothing in this phase crosses a
browser/native boundary.

### Phase 5 — Mounted browser View and typed DOM events (DONE)

- mount a Vut `View` into a real browser `Element` using the existing WASM host;
- marshal typed mouse, input/change, keyboard, focus, submit, reset, select, and
  invalid event snapshots to their declared Vut callback signatures;
- keep DOM event control synchronous and avoid exposing JS objects in Vut memory;
- attach/detach listeners with deterministic callback lifetime and host shutdown;
- keep event registrations out of both HTML attributes and the native SSR value.

Exit: a real browser dispatches the supported form events into typed Vut
callbacks; prevention/propagation controls affect the original event; refresh and
unmount leave no duplicate or dangling listener.

The host is `wasm32-unknown-unknown` on the browser main thread. The shared
`crates/vut-codegen/src/wasm/browser_host.js` loader and
`browser_view_host.js` use standard DOM `addEventListener`/
`removeEventListener`. `mountView(container, render)` builds the tree, maps only
the event fields declared by `vutnet.view.event_types`, and owns listener callback
wrappers for the mount. A per-dispatch opaque control ID associates the active
Vut event snapshot with the original DOM event for synchronous
`prevent_default()`/`stop_propagation()` calls. No raw DOM event or pointer is
marshalled. Invalid events use capture phase because the browser does not bubble
them.

`refresh()` disposes the previous tree's listeners and callback wrappers before
replacing its nodes, restores focus/selection without synthetic dispatch, and
rebuilds listeners exactly once. Callback-triggered refresh is queued as a
microtask, after the current DOM propagation phases finish. `dispose()` removes
the tree and listeners and disposes Vut callback wrappers after an in-flight
synchronous invocation exits. `api.dispose()` drains all mounts and listeners.
The callback-free `ViewRender` native projection keeps SSR independent from
browser closures.

The real Chromium E2E in `crates/vpm/tests/vutnet_view_events_e2e.rs` covers
button click, input value/checked, textarea input/select, select value, option
click, keyboard key/code/repeat, focus/blur, label click, form submit/reset,
`prevent_default`, invalid-event capture, propagation stop, repeated refresh, and
unmount cleanup. The package-boundary fixture in
`crates/vpm/tests/vutnet_view_e2e.rs` checks inline lambda inference and rejects
wrong handler signatures. These browser handlers use typed callback wrappers
from the existing WASM host. A separate reactive browser E2E also verifies a
capturing event callback that writes `web.state`, plus retained renderer/update
callbacks. These tests establish the exercised mounted-View path, not general
async or foreign-thread callback entry.

### Phase 6 — Mounted View state subscriptions (DONE; typed bindings + eager compatibility)

- track cells read while invoking a typed zero-argument View renderer;
- retain/release each mount's dependency set and detach it on refresh, unmount,
  and host disposal;
- coalesce writes in a frame, then rerender through the existing View builder;
- verify captured state callbacks and repeated updates in a real browser.

Exit: a browser cell write updates mounted View output, and unmount releases
subscriptions and renderer/event callbacks.

Status: DONE for dependency tracking and typed fine-grained bindings. The WASM cell bridge
tracks reads during synchronous binding evaluation, isolating initial child
construction from ancestor renderers. `cell.set()` notifies subscribers; a shared
`requestAnimationFrame` scheduler coalesces affected bindings. `mountView(container, render)` accepts a typed Vut `fn() -> View`
callback or a typed exported zero-argument Vut function that returns `View`.
Each successful refresh swaps its retained dependency set and patches matching
DOM nodes. The renderer preserves its existing listener/callback cleanup.
This is browser-main-thread behavior; SSR does not subscribe. Initial hydration
and unique-ID list moves are implemented. Typed binding subscriptions are implemented.

### Phase 7 — Examples and release verification

- provide a package example with components, layout/slots, SSR, and HTTP output;
- run VPM package-boundary, compile-fail, escaping/security, ownership, and real
  HTTP E2E gates for features actually implemented;
- update this roadmap and the existing overview/response/package specs when a
  phase changes public behavior or status.

Exit: advertised capabilities have end-to-end evidence, including hydration and
typed fine-grained binding evaluation without enclosing Page reevaluation.

Status: DONE for the implemented SSR surface. The standalone VPM app in
`../../vutnet-examples/view-ssr/` demonstrates typed components, required/fallback slots,
Page-owned layout, typed page data, `response.render`, HTML escaping, inline and
external CSS, `Controller(Context) -> Response`, and real Hyper delivery. The
VPM integration test runs example and CSS fixture sources through a real
VPM-built package and HTTP client.
The typed Page E2E covers no-data Pages, layout/slots, data iteration, static
Page/data mismatch, unsafe URL and direct/nested layout errors to HTTP 500,
repeated rendering, response media type, and View ownership. Phase 5's real Chromium E2E verifies
typed DOM form events and listener lifecycle. The older in-process
`InteractiveView` wrapper is not a separate mount API; verified state
subscriptions are owned by browser `mountView`, with no standalone public
subscription handle.

### Phase 8 — SSR hydration (DONE, with incremental DOM commits)

`response.render_hydratable(Page[, data])` emits versioned bootstrap JSON and the
shared plan. Browser `hydrateView(container, Page)` and
`hydrateView(container, Page, {fallback: "replace"})` validate the evaluated
Page and DOM markers, then attach typed callbacks and state subscriptions to the
existing nodes. Initial hydration preserves DOM identity. `if/else`, `for`,
components, nested components, layouts and slots are represented by their
evaluated flattened tree and validated before attachment. A state write after
hydration evaluates subscribed bindings and patches their retained nodes/regions,
using unique IDs for reorderable list/component roots. Eager reads retain the
full-render compatibility path. Production client bundling uses the shared dev bootstrap. See
[`13-ssr-hydration.md`](13-ssr-hydration.md).

### Phase 9 — Production assets and incremental DOM (DONE)

Run/build stage the native app and VPM-generated browser registry/client
together; start validates and serves the release bundle without compiling. See
`11-cli.md`.

Updates preserve matching DOM nodes and typed callback/subscription cleanup.
Use unique, stable IDs on reorderable item/component roots:

~~~vut
fn Row(name: str) -> View:
  view:
    li(id = "row-$name"):
      input(value = name)
~~~

The input is matched within its retained row, so moving that row preserves
its draft/focus. Repeated rows without IDs are positional. An unchanged
declarative input value preserves edits; a changed value resets the live value.
Removed nodes/listeners are cleaned up. This preserves DOM identity, not
component-local state recreated during each region render. Typed bindings avoid
whole-View evaluation; detached candidate construction occurs only for changed
children regions or eager compatibility renders.

### Phase 10 — Fine-grained typed bindings (DONE)

`text(fn() -> str)`, `bind_attrs(fn() -> list[Attribute])` and
`children(fn() -> View)` retain per-binding cell dependencies and owned callbacks.
Real Chromium tests count renderer/component/binding executions, verify conditional
dependency switching, parent-before-child cancellation, keyed controls, nested
regions, raw-text elements, recoverable traps/invalid attrs and cleanup.
Dev/run/build/start hydration E2E uses these bindings with typed server data.
See `14-fine-grained-view.md`.

## 10. Verification matrix

Implementation phases should add focused checks at the point each behavior is
introduced:

- builder/tree: root and nested order, text, fragment, child component,
  multiple roots, if/else, loops, and value ownership;
- compile behavior: wrong props, wrong child value, invalid attribute types,
  and unsupported string-child shorthand fail cleanly;
- slots: fallback, supplied content, required slot, nested layout, duplicate
  supply/declaration and unknown slot;
- rendering: text/attribute escaping, boolean attributes, void tags, invalid
  names, URL schemes, stable output, and errors without unescaped fallback;
- CSS: raw `<style>` text preservation, media/id/class/custom attrs, empty
  elements, Unicode and special characters, `</style` rejection across text
  nodes, nested Elements rejected, void `<link>` attrs/serialization and child
  rejection;
- ABI: nested list/data identity across VPM packages, retain/release, repeated
  render, error cleanup, and no private ABI dependency;
- callback host ABI: named callback return/invoke/dispose, repeated calls,
  module/signature mismatch, and unsupported nominal argument diagnostic;
  captured cell state is covered through the mounted View E2E, while arbitrary
  managed capture shapes still need separate coverage;
- call-site identity: deterministic source IDs, direct tracked-wrapper
  propagation, direct callable-value rejection, native/WASM parity, marker
  versioning, per-site occurrence plan, element/text markers, script-safe typed
  bootstrap data, no-data bootstrap, and unchanged default SSR output;
- events: exact tag/signature mapping, typed event data, `prevent_default`,
  propagation, real-browser input/change/key/focus/submit/invalid dispatch,
  repeated refresh without duplicate listeners, and mount cleanup;
- state: captured `web.state` reads and writes, frame-coalesced rerender, and
  subscription teardown on unmount; retained node identity, control drafts and ID-keyed list moves;
- integration: typed Page / typed data → Page-owned layout/slots → public native
  SDK renderer → `Response` → real HTTP client for SSR; also no-data Page,
  ordered `head` metadata/stylesheet/inline CSS, public CSS asset serving,
  compile-fail type mismatch, repeated calls and HTTP 500 on render errors;
  `response.render_hydratable(Page[, data])` additionally verifies bootstrap
  version, generated or custom JSON encoding, the flattened plan, escaping and limits.
  Browser hydration verifies typed Page data, DOM/plan mismatch, explicit
  replacement fallback, existing node identity, event/state attachment and
  cleanup in a real headless browser.
  Phase 5 additionally runs a real Chromium DOM click
  into typed View callbacks. The form-event browser E2E also covers real DOM
  events and listener cleanup.

Phase 0 syntax probes, Phase 2/3 managed-ABI and real-socket evidence, Phase 4
event-value lifecycle evidence, the root nominal callback/DOM E2E, typed browser
View mounting/form events, and the Phase 7 standalone VPM/Hyper example are
recorded above. State semantics remain described in Section 7.2. Templates remain
out of scope.

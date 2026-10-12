# Fine-grained View bindings

## Contract

State writes invalidate only subscribed binding callbacks. A callback is a typed
Vut closure, not JavaScript source, a string template, or a raw runtime handle.
The existing cell notification, WASM callable bridge, hydration matcher and DOM
patcher remain canonical. Core has no DOM-specific semantics.

The explicit binding boundaries are:

- `text(value: fn() -> str)`: one existing DOM Text node.
- `bind_attrs(value: fn() -> list[Attribute])`: attributes of the last element
  added to the current scope, including void elements. As the first operation
  inside an element body it binds that element's own attributes. The callback contributes
  attributes after the element's static attributes, using the same validation and
  serialization pipeline. Duplicate attribute names follow the existing validator.
- `children(value: fn() -> View)`: the children of the current element. It is
  exclusive with ordinary children in that scope. The element stays connected;
  only its children are reconciled. This is the boundary for reactive conditions,
  loops, fragments and components, including nested child bindings.

Eager `text(str)` and eager tag arguments keep their existing semantics. A cell
read outside a binding retains the compatibility full-render subscription; it
cannot be inferred from a string after evaluation. Explicit bindings never add
those reads to their ancestor renderer's collector. This distinction is part of
the API, not an implicit conversion or compiler special case.

## Evaluation and ownership

SSR evaluates each binding once to produce ordinary HTML. The native rendering
projection carries evaluated nodes and attributes only, never binding callbacks.
Browser construction evaluates initial values in isolated dependency scopes.
After matching existing SSR DOM, the mount invokes binding callbacks in separate
collectors, checks their initial results against the evaluated nodes, and commits
subscriptions. Binding callbacks must be pure: initial browser construction and
attachment both evaluate them, and a conditional dependency can be re-evaluated
on later writes. They must not create new state identities, mutate cells or do IO.
It never rebuilds the root on a bound cell write.

Each binding owns its closure and one retained reference per observed cell.
Successful evaluation replaces its dependency set; failed evaluation releases new
references and preserves the previous DOM/dependencies. Writes are coalesced into
one animation frame per mount. Dirty child regions run before their descendant
bindings; removed descendants cancel pending work and release callbacks/cells.
Reads inside event handlers do not subscribe. Events do not unconditionally
schedule a root refresh.

Text changes update `nodeValue`. Attribute changes reuse validation and the
control-state lifecycle. Child changes reuse keyed DOM reconciliation and the
existing event/mount lifecycle, without rerunning the enclosing Page, Layout,
static siblings, or unrelated Components. Nested mounts attach to existing child
nodes and own their local callbacks. Unmount and host disposal are idempotent.

Recursive callback result schemas use canonical nominal references in generated
browser metadata, retaining static signatures without expanding recursive data
forever. This is generic WASM interop support, not a Vutnet-only ABI.

## Example

~~~vut
count = state[int](0)
render = fn() -> View:
  view:
    h1:
      text(fn() -> str:
        "Count: $(count.get())"
      )
    button(on_click = fn(event):
      count.set(count.get() + 1)
    ):
      text("+")
~~~

## Verification

Require real browser tests that count Page and individual binding executions,
verify unrelated bindings do not run, verify conditional dependency changes,
coalescing, nested region cleanup, keyed controls, SSR node identity, custom
bootstrap data, recoverable binding errors, and unmount/host cleanup. Do not use
unchanged DOM alone as proof of fine-grained evaluation.

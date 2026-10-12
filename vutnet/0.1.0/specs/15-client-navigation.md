# Client navigation

Status: **implemented and covered by real-browser E2E** as a progressive enhancement
for hydratable SSR pages and eligible same-origin forms.

## Contract

Normal links remain ordinary HTML anchors. With the Vutnet browser client loaded,
eligible same-origin links use client navigation while preserving the current
Window and Document. Every destination is still requested from the running
Vutnet server and handled by the existing flattened Router, Context, Controller
and Response path. Vutnet does not build a second browser router.

The browser fetches the target as HTML. The response must contain one supported
Vutnet hydration bootstrap and the same versioned browser client URL. The browser
validates the bootstrap protocol before committing the SSR response. It
disposes the old mounted View and its listeners/subscriptions, retains the live
`Document`, `<html>`, `<head>` and `<body>` nodes, synchronizes shell
attributes, and replaces the head/body children with the fetched SSR content.
It reuses unchanged external stylesheet nodes when their resolved URL and
presentation attributes match, rebinding their per-View hydration marker to the
destination Page. It resolves the destination Page through the generated typed
registry and calls `hydrateView` against the committed SSR DOM. It never replaces
the document root or starts a document navigation for an eligible hydratable link.

The response URL becomes a History API entry only after the SSR response is
available. The new page initializes its client state from that response's typed
bootstrap data. It does not carry old page cells across routes. The server render
remains the source of truth for both URL matching and initial HTML/state.

## Eligible links and forms

Vutnet intercepts an unmodified primary-button click on a same-origin HTTP(S)
`<a href>` with no `download` and no non-`_self` target. External links,
modified clicks, downloads, new-window targets and same-document fragment links
remain native.

The delegated submit handler enhances same-origin HTTP(S) forms targeting
`_self` when the submit event has not already been cancelled by application code.
GET forms serialize successful controls and the submitter into the action query.
POST forms support `application/x-www-form-urlencoded` and `multipart/form-data`;
unsupported encodings and targets retain native browser submission. Browser
constraint validation still runs before the submit event. The `submitter` name,
value and per-button action/method/encoding overrides are preserved.

A successful form response uses the same SSR/hydration commit path as a link.
POST handlers should use Post/Redirect/Get (303) so the visible history URL is a
normal GET page and refresh/back behavior stays conventional. A direct hydratable
POST response can be committed, but refresh then requests that URL with GET; apps
should use 303 when that is not equivalent. A direct unsupported POST response
is reported through the navigation error event and left to application recovery;
Vutnet does not silently repeat a POST because that could duplicate side effects.

While a navigation is pending, the current page remains visible and
`<html data-vutnet-navigation-state="loading">` is set. The browser dispatches
`vutnet:navigationstart`, `vutnet:navigationend`, or
`vutnet:navigationerror` on `document`, with `url`, `method`, and `source`
(`link`, `form`, or `history`) in `event.detail`. The loading attribute is
removed on completion or failure. Apps can use it for a progress indicator.
Competing navigations abort the older fetch and stale responses are ignored.

## Prefetch and response cache

Prefetch is opt-in through the ordinary anchor relation `rel="prefetch"` and
starts on pointer hover or keyboard focus. It uses same-origin GET with the
current credentials and the same Router/Controller SSR path. The in-memory route
cache accepts only successful hydratable HTML that explicitly has a positive
`Cache-Control: max-age`; `no-store`, `no-cache`, and `Vary: *` responses are
never retained. Entries are keyed by URL without the fragment, capped at 20,
and capped at 30 seconds even when the server grants a longer freshness period.
POST submission invalidates prefetched pages. Leaving `Cache-Control` unset keeps
pages uncached and avoids speculative requests unless the link opts in.

## Fallback and safety

For GET navigation, the browser performs a normal document navigation when the
response is not HTML, is an attachment, has a Content Security Policy, contains
executable page scripts, lacks the supported hydration markers, or points to a
different client build. Network, parsing or hydration failures also fall back to
normal navigation. For POST, a followed redirect is safe to fall back to as a
GET; a direct unsupported response is reported and does not replay the POST.
This preserves behavior for SSR-only routes without risking duplicate side effects.

The bootstrap query contains a build identity derived from the staged client
WASM, JavaScript bindings and Vutnet browser runtime. The browser asset URLs use
that identity so a page left open across a successful dev rebuild cannot hydrate
new SSR with an old WASM client.

Back/Forward uses `popstate` and fetches the selected URL through the same server
route path. Vutnet restores known scroll positions within the current browser
session, scrolls new entries to the top, and honors a target fragment after
hydration. Hash-only links keep native browser behavior.

When JavaScript is disabled, the anchors use regular SSR page requests. Normal
`response.render` pages remain non-hydratable and continue to work through full
document navigation.

## Deliberate V1 limits

- Unsupported form targets/encodings remain native; multipart form navigation
  still obeys the server's configured request-body limit.
- POST should use 303 Post/Redirect/Get. Direct hydratable POST output can be
  shown, but refreshing that URL sends GET; an unsupported direct POST response
  cannot safely trigger a second request automatically.
- No optimistic state or transfer of old page-local cells is provided; each
  destination initializes from its typed SSR bootstrap.
- Pages containing executable scripts or CSP use full document navigation.
- Prefetch is opt-in and requires an explicit cache freshness policy.

## Verification

`crates/vpm/tests/vutnet_dev_hydration_e2e.rs` runs against a real VPM-built
Vutnet server, real Hyper sockets and headless Chromium. It checks a nested
`UseRouter` link to a typed, auto-encoded Page without a document request,
GET and POST/303 form submissions, submitter values, pending lifecycle events,
opt-in prefetch/cache reuse, preservation of the live document shell and
stylesheet, hydration and events after navigation, Back/Forward, and fallback
to full navigation for a non-hydratable HTML route. The same flow runs for dev,
run and packaged start builds, including a typed bootstrap decoder.

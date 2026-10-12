# Vutnet 0.1.x — Static files and multipart

Status: Phase 6 DONE. The combined VPM-built Vut app passes real HTTP socket
acceptance in `crates/vpm/tests/vutnet_static_multipart_e2e.rs`; the existing
HTTP/router/middleware suites also pass regression.

## Static files

```vut
app.static_files("/assets", "./public")
```

`fn App.static_files(prefix: str, directory: str) -> void` adds a startup mount.
Vut reserves `static` as a keyword and its member parser does not accept that
keyword, so `app.static(...)` is not valid current syntax. This phase uses the
descriptive identifier `static_files` without a language change.

The prefix must start with `/`. One trailing slash is normalized; duplicate
prefixes, query/fragment, percent escapes, control characters, repeated slash,
dot segments, backslash and colon are invalid. `/` may mount the root namespace.
`directory` is trusted application configuration, resolved relative to process
working directory and canonicalized/opened before listening. Invalid roots or
mount configuration make `listen` return `-7` without exposing filesystem paths.

Ordinary Router path matches take priority, including their 405 response. If
no Router path matches, the longest matching static prefix wins; matching is
segment-boundary aware. Mount/route registration order does not affect this
precedence. A missing file does not fall back to another mount or controller.
Static responses bypass global and scoped Vut middleware, like existing native
404/405 responses; no Vut callback runs to serve a static file.

Only GET and HEAD may open a file. Other methods return 405 with `Allow: GET,
HEAD`. HEAD sends GET metadata/Content-Length without a body. Directories and
missing files return 404, including `/assets/`; there is no listing, index.html
fallback or redirect. Permission failures return 403 and internal failures 500,
using fixed messages without path details.

Each request suffix is percent-decoded exactly once component by component.
Reject dot segments, decoded separators, control/NUL bytes, drive/ADS colons,
absolute paths, Windows device names and trailing dots/spaces. Every component
is then opened beneath a retained `cap_std::fs::Dir` capability; request paths
never become ambient filesystem opens. The capability layer protects against
symlink/reparse escapes during resolution rather than relying only on a
check-then-open canonicalization. The configured root may itself be a trusted
symlink; its resolved directory is the authority boundary. Hard links created
inside that directory are inside the configured filesystem authority.

`mime_guess` chooses the extension media type, with octet-stream fallback.
GET owns an opened regular file and streams it with Tokio/ReaderStream in 64 KiB
chunks. Capability opens run on the blocking I/O pool. Content-Length is the
opened file's size; concurrent truncation/read failure closes the stream rather
than returning path details after headers have been sent. Static content should
be replaced atomically by the trusted application/operator. There is no Range,
conditional caching, compression or public Vut streaming Response API.

### Serving CSS referenced by a View

Vutnet already serves public assets through `App.static_files`; View does not
start another server or automatically mount a `public` directory. For the
conventional layout below, mount the project `public` directory at the URL root:

~~~text
project/
├── src/
└── public/
    └── styles/
        └── app.css
~~~

~~~vut
app.static_files("/", "./public")
~~~

Then the existing `vutnet.view` element can reference that file:

~~~vut
link(href = "/styles/app.css", rel = "stylesheet", link_type = "text/css")
~~~

`GET /styles/app.css` maps to `public/styles/app.css` under the existing
capability-checked static-file handler. The root mount is explicit and does not
provide an `index.html` fallback; Router matches keep their documented priority.
For a narrower mount, for example
`app.static_files("/styles", "./public/styles")`, the same URL path is
available. `link()` only emits HTML and does not copy, bundle, hash, or rewrite
the asset.

## Multipart

```vut
import vutnet.context at Context
import vutnet.multipart at Multipart, MultipartFile, MultipartError

fn Context.multipart() -> result[Multipart, MultipartError]
fn Multipart.field(name: str) -> str?
fn Multipart.fields(name: str) -> list[str]
fn Multipart.file(name: str) -> MultipartFile?
fn Multipart.files(name: str) -> list[MultipartFile]
fn MultipartFile.name() -> str
fn MultipartFile.filename() -> str?
fn MultipartFile.content_type() -> str?
fn MultipartFile.bytes() -> bytes
fn MultipartFile.size() -> int
```

The Rust adapter parses multipart/form-data with `multer` once for a matched
Router request before constructing Context. Middleware and controllers share
an owned snapshot; repeated `ctx.multipart()` calls do not parse again. All
matched routes, including malformed multipart requests, keep normal middleware
ordering and short-circuit behavior.

Errors are `not_multipart`, `missing_boundary`, `invalid_boundary`, `malformed`,
`invalid_text`, `invalid_disposition`, or `too_many_parts`. `ctx.multipart()`
returns the typed error; middleware/controller should map bad form input to a
400 Response. Parsing does not abort, fabricate an empty success, or return
partial forms. Ordinary requests remain usable and get `not_multipart` only
when this accessor is called. A controller that ignores malformed multipart
can respond normally; error-to-status policy is explicit application logic.

Part Content-Disposition must be form-data with a parsed name. A filename
parameter (including an empty filename) classifies the part as a file; otherwise
it is a strict UTF-8 text field. Uploaded bytes never undergo text decoding.
Content-Type is optional file metadata. Empty fields/files are preserved.
Multer normalizes repeated part header names to the last wire value; validation
and metadata extraction use that same value. Repeated form parts remain distinct.
`field`/`file` return the first matching value; `fields`/`files` preserve all
matching values in wire order. Names are case-sensitive. Filename metadata is
untrusted and may contain paths; the framework never writes it to disk.

All uploads remain in memory. The server enforces `App.with_max_body_bytes`
(default 1 MiB) for Content-Length and streamed/chunked bodies before parsing or
Vut dispatch; oversized bodies return 413. Multer also receives finite stream
and field bounds. Multipart is capped at 1024 parts to bound metadata overhead.
Memory is proportional to the configured body bound plus bounded part metadata;
the raw body and owned parsed/ABI copies can coexist. No temporary upload files
are created. The limit is per request, not a process-wide connection quota.

## Native boundary and ownership

`vutnet_app_listen_v3` adds the typed `list[StaticMount]` startup argument;
v1/v2 exports retain their original signatures with no static mounts. Its
existing callable-bearing route argument selects general public value transport.
Multipart types are nominal data with typed lists, strings, bytes and booleans.
Optional metadata uses private presence flags plus strings in transported data;
public accessors expose `str?` in Vut. No optional/result value needs to cross
the callback ABI for this feature. Context copies retain the same multipart
snapshot through middleware. SDK owned handles release nested values on every
completion/error path. Files and directory capabilities are Rust owners and
close on completion, disconnect or shutdown.
After Context construction, Rust drops its raw request/parsed form before
submitting the callback, retaining only the independently owned Vut snapshot.

No compiler/runtime/VPM or public Native Callback ABI extension is required.
The existing serialized callback and abort-on-unhandled-Vut-trap limits remain.

## Acceptance

Native tests verify URL/Windows path rejection, duplicate/invalid configuration
and directory capability symlink/junction confinement. A real VPM-built Vut app
must serve static text/MIME/binary/HEAD, reject traversal/methods/directories,
and run typed multipart upload controllers through middleware over real sockets.
Tests cover repeated names/files, metadata, exact binary bytes, empty parts,
malformed/truncated requests, missing/invalid boundaries, body limits, repeated
requests and separate connections. Existing HTTP/router/middleware E2E remains
a required regression gate.

## Complete application

```vut
import vutnet
import vutnet.router at router, Router
import vutnet.method as method
import vutnet.context at Context, Next
import vutnet.response at Response
import vutnet.response as response

fn Trace(ctx: Context, next: Next) -> Response:
  match next(ctx).with_header("x-upload-app", "yes"):
    ok(value): value
    err(_): response.send(500, "Invalid header")

fn Upload(ctx: Context) -> Response:
  match ctx.multipart():
    ok(form):
      match form.file("avatar"):
        null: response.send(400, "Missing avatar")
        file: response.send_bytes(200, file.bytes())
    err(_): response.send(400, "Invalid multipart/form-data")

fn RootRouter() -> Router:
  router:
    Route(path = "/upload", controller = Upload, method = @[method.Method.post])

fn main() -> int:
  app = vutnet.app()
  app.static_files("/assets", "./public")
  app.use(Trace)
  app.router(RootRouter)
  app.listen(3000)
  0
```

`GET /assets/index.html` streams `./public/index.html` without Trace.
`POST /upload` with multipart `avatar` invokes Trace/Upload and returns exact
file bytes. Donix adds a browser form plus typed name/file metadata example.

## Validation record

Windows checkout validation passed native `check`, `fmt --check`, six adapter
tests, ten server tests and both native `clippy --all-targets -- -D warnings`.
VPM `check`, test lint, 103 library tests, 11 router signature tests, the existing
HTTP E2E and combined static/multipart E2E passed. Library tests use the matching
checkout runtime libraries; the installed older runtime caused two failures
before those environment overrides. Donix build/fmt/lint and real HTTP static
HTML, HEAD CSS and multipart name/avatar metadata checks passed.
Donix's existing `vpm test` JSON smoke runner still times out after 60 seconds;
this previously recorded test-process lifecycle issue remains outside the
static/multipart implementation. It does not block the real HTTP acceptance.

Whole-workspace `cargo fmt --all -- --check` still reports pre-existing leading
blank lines in `crates/vut-lexer/src/lexer/tests.rs` and
`crates/vut-parser/src/parser/tests.rs`; this phase does not modify those files.
VPM/native scoped formatting gates pass. No sanitizer/heap-counter claim is
made: repeated requests/connections and graceful drain test behavior/ownership,
while public managed-handle counters are unavailable in the current SDK.

# Vutnet CLI

## Ownership and executable entry

The `vutnet` command belongs to the `vutnet` VPM package. Its `[[bin]]` is a
Vut source entrypoint, built and installed by VPM like other packaged Vut
commands:

```toml
[[bin]]
name = "vutnet"
path = "src/bin/vutnet.vut"
```

The executable starts in `src/bin/vutnet.vut`; Vutnet's command table and
argument dispatch live in `src/cli.vut`. The supported commands are
`new`, `dev`, `run`, `build`, `start`, `routes`, and `info`, with `help` and
`--version` support. Vut owns command selection and argument shape. Command
implementation calls typed, named native entrypoints for the operations that
need filesystem watching, child-process control, project inspection, or
structured terminal output.

Rust implementation source lives under `native/src/cli/` and is compiled as
part of the Vutnet native library. `native/src/cli_ffi.rs` exposes the typed
operations to Vut through the public managed native-import ABI. The native
source stays inside VPM's native build fingerprint, so backend changes
invalidate the native artifact correctly. There is no separate Rust CLI
executable and no Vutnet-specific behavior in VPM, the compiler, or runtime.
For this CLI-only binary, native build support supplies the declared fallback
schema versions for framework types omitted from the compiler manifest; normal
Vutnet applications use exact versions emitted for their reachable managed
imports.

The native build script checks its version against `[package].version` in the
Vutnet `vpm.toml`. That package version drives `vutnet --version` and generated
project dependencies.

## Commands

```text
vutnet new <name>
vutnet dev [--verbose]
vutnet run
vutnet build
vutnet start
vutnet routes [--json]
vutnet info
```

The Vut dispatcher rejects missing or extra command arguments before calling
native code. Rust owns the system-level work and terminal presentation. The
CLI invokes the installed `vpm` executable for project creation, dependency
resolution, and builds; it does not link VPM's Rust library or the Vut
compiler.

## `new`

`vutnet new <name>` delegates project creation to `vpm new`, writes a Vutnet
router/controller example, pins the Vutnet VPM dependency to the package
version, creates a comment-only `vutnet.toml`, and runs `vpm update`. The
generated config parses as the default configuration, so Tailwind stays
disabled until the user adds a `[tailwind]` section. If resolution fails, the
scaffold remains available for repair. Existing destination files are never
overwritten.

## `dev`

`vutnet dev` is a persistent development supervisor. The command is dispatched
by the Vut `[[bin]]`; its native Rust implementation discovers the nearest
project, validates the manifest, starts the existing OS filesystem watcher,
and builds the application through `vpm build`. It does not rerun `vpm run` on
source changes.

The supervisor prints the Vutnet banner once per `vutnet dev` session. The first
successfully started child prints its ready details without a second banner.
After a source change, rebuild lifecycle output is limited to the changed path,
`Rebuilding...`, and `Rebuilt in ...`; the replacement child's ready/stopped
messages are suppressed. Request logs and startup/runtime errors remain visible.

By default, request logs include application/router responses and errors from
the static-file handler, while successful static-file responses (2xx/3xx) are
omitted. `vutnet dev --verbose` includes every HTTP request. The server child
receives this setting through a private environment marker; the request logger
gets the static classification from the actual handler selected after Router
precedence, never by guessing from URL extensions.

The supervisor compiles a candidate before stopping the running server. A
successful VPM build is copied to a unique `.vut/dev/app-<pid>-<generation>`
artifact (with the platform executable suffix) and launched as a managed
process tree. This keeps a running executable out of VPM's fixed build output,
including on Windows where a running executable cannot be overwritten. Only
one build/replacement transaction runs at a time. Changes arriving during a
build mark the source revision dirty; the stale candidate is discarded and the
queued, debounced save triggers a build of the latest source.

Compilation failure is recoverable. On initial failure, the supervisor logs
the compiler output and waits for another change without exiting. After a
successful build is running, later compile failures leave that last-known-good
server running. A successful candidate stops and reaps the previous server
tree before binding the replacement, then removes the obsolete generation
artifact. If the new server cannot start, the supervisor remains alive and
waits for another change. It does not automatically restart a server that
crashes at runtime; it reports the exit status and waits for a save/build.

Ctrl+C cancels an active VPM build, stops the watcher, terminates and reaps the
server process tree, and removes the supervisor-owned generation artifact.
Build diagnostics and lifecycle messages append to the existing terminal log;
the terminal is never cleared during reload. The watcher observes project Vut
sources, manifests, and local path dependency roots while ignoring generated
build output, VPM caches, and `.vut/dev` artifacts.

### Automatic browser hydration client

A project opts into browser hydration by using
`response.render_hydratable(Page[, data])`. No browser table in
`vutnet.toml`, consumer `[[bin]]`, client entry source, factory or decoder
function is required. Ordinary `response.render` remains SSR-only.

The VPM checked program is the source of the typed registry. For every statically
named Page passed to `render_hydratable`, VPM records its stable compiler
call-site ID and validates that the Page is a non-generic function with signature
`fn() -> View` or `fn(Data) -> View`. A data argument is one concrete public
nominal `data` value that implements `json.Encodable`; variadic or dynamically
computed Page callables are rejected. VPM writes a generated temporary Vut module
with a concrete wrapper for each registration plus `client.registry.json`.
A public static `Data.from_json`, if defined, is called through a generated
typed decoder wrapper; otherwise the generated nominal field schema decodes the
ordinary bootstrap data.

For each dev generation, Vutnet builds the native application and invokes VPM's
typed hydration-registry build for `wasm32-unknown-unknown`. VPM compiles the
same checked source graph, selecting its generated registry entrypoint and
retaining the typed Page wrapper exports as browser bindings. It emits
`vutnet-client.wasm`, JavaScript bindings and the registry without changing the
project's source tree. Vutnet stages these with bootstrap/navigation scripts in
that generation's `.vut/dev/generation-*/browser/`; a failed client build
rejects the candidate and leaves the last-known-good server and assets active.
The private `/__vutnet/` mount serves the selected generation's files.

Hydratable SSR emits a versioned bootstrap containing its Page call-site ID and
typed initial data. The loader validates the registry, selects the matching
generated Page wrapper, and calls `hydrateView(document.documentElement, Page)`
against existing SSR DOM. The Page function runs with the same typed server data;
its View is checked against the shared hydration plan before event/state
subscriptions are attached. Client navigation uses the same registry selection
for every fetched SSR response. It enhances same-origin GET/POST forms that target
`_self` and use supported encodings, while preserving submitter values; POST
handlers should return a 303 redirect. Pending navigation is exposed through
document events and a temporary root data attribute. Links opt into prefetch with
`rel="prefetch"`; the cache requires explicit `Cache-Control: max-age` and is
bounded to 20 entries and 30 seconds.

`vutnet dev`, `run` and `build` automatically build and stage this client.
When no hydratable Page registrations exist, VPM emits no browser client.
Production `start` serves the previously packaged registry and assets without
invoking VPM or reading browser config. Compile the client as part of the
candidate before replacing a running dev server. See `13-ssr-hydration.md` for
the browser data and lifecycle contract.

## `run`, `build` and `start`

`vutnet run` builds the debug application and then runs it with the project root
as its working directory. `vutnet build` builds release native and automatically
discovered browser targets. Both commands stage a complete bundle in
`build/{debug|release}/vutnet/`: executable, versioned manifest and optional
`browser/{client.wasm,client.js,client.registry.json,bootstrap.js,navigation.js,build-id}`.
A failed client build leaves the previous bundle intact. `start` validates the
release bundle and serves its manifest/assets without invoking VPM or rebuilding
browser assets. Stop a running bundle before rebuilding on Windows. Project-owned
public/static directories remain relative to the project root and must be
deployed with it.
When `[tailwind]` is configured, CSS is published only after all builds succeed.

`vutnet.toml` is an optional, strict, typed Vutnet configuration at the project
root, separate from VPM's `vpm.toml`. Unknown keys, malformed TOML, unsupported
Tailwind versions and invalid project-relative paths are reported with the
config path and parser/validation detail. No Vutnet settings are written to
`vpm.toml`.

Tailwind is enabled by the presence of `[tailwind]`:

```toml
[tailwind]
version = "4.1.14"
input = "src/app.css"
output = "public/assets/app.css"
```

The version is exact and pinned; paths resolve from the project root. Vutnet
asks the generic VPM tool resolver for the exact host executable once per
command/session, then invokes that cached standalone binary directly. Neither
Node.js, npm nor npx is required. `vutnet dev` resolves the executable once for
the whole supervisor session, not on every rebuild.

## `routes`

`vutnet routes` builds and runs the application with the route-dump process
marker. The Vut application still constructs and flattens its real Router. The
native server adapter reads the flattened typed route list through the public
Native Callback ABI, emits JSON, and exits without binding a socket. The CLI
renders aligned method/path rows or uncolored JSON with `--json`; it does not
parse Vut source or implement a second router.

## `info` and terminal output

`vutnet info` reports the CLI/package version, consuming Vutnet dependency,
project manifest and entry paths, host, and installed Vut/VPM versions when
available. Missing external version commands display `unknown`.

One Rust terminal UI module owns color and formatting. It detects TTY, honors
`NO_COLOR`, and avoids ANSI sequences in non-TTY and JSON output. Native server
request/error logging continues through Vutnet's existing logger.

## Development and release status

The consumer smoke project at `../../vutnet-examples/cli-smoke/` exercises
the Vut dispatcher through the native ABI; examples are kept outside the
publishable Vutnet package so their nested VPM manifests are not archived. The
official executable is built from the Vut `[[bin]]` entry through VPM. After
Vutnet is registry-ready, users install the command with:

```sh
vpm install vutnet
```

The server engine is now an internal Cargo dependency within the single
Vutnet VPM package; there is no local VPM path dependency or nested server
manifest. A registry release is still blocked for isolated native artifact
builds: `native/Cargo.toml` references the Vut monorepo's `vut-native-abi`,
`vut-native-sdk`, and `vut-runtime` crates by relative paths. Those crates must
be provided in the trusted artifact build context or published and consumed as
versioned Rust dependencies. Then validate the VPM publish plan and all supported
native artifact targets before release.

# Tailwind CSS integration

## Configuration boundary

Vutnet framework configuration lives in optional `vutnet.toml` beside
`vpm.toml`. VPM package/dependency data remains in `vpm.toml`; the two files
must not absorb each other's concerns. The Vutnet loader uses strict typed TOML
deserialization and reports the config path plus the parser or validation
reason. Unknown keys are errors.

V1 supports only this section:

```toml
[tailwind]
version = "4.1.14"
input = "src/app.css"
output = "public/assets/app.css"
```

The presence of `[tailwind]` enables Tailwind; there is no `enabled` flag.
`version` is an exact supported version, never `latest`, a range or wildcard.
`input` and `output` are nonempty project-relative paths resolved from the
project root. Absolute paths and lexical traversal outside the project are
rejected. When the file or section is absent, Tailwind is disabled and the
ordinary Vutnet flow is unchanged.

## Tool acquisition boundary

Vutnet reads the config and asks `vpm tool resolve tailwindcss <version>` for a
host executable. VPM owns the generic tool catalog, OS/architecture selection,
download, trusted SHA-256 verification, shared cache, corrupt-cache recovery
and executable permissions. Vutnet invokes the resolved executable directly;
it does not implement a downloader or require Node.js/npm/npx. Tool resolution
happens once per `run`/`build` command and once per `dev` supervisor session.
The resolved executable path is reused for every rebuild in that session.
When a cache miss downloads a known-size artifact, `vutnet` forwards VPM's
percentage updates into its Tailwind resolving status. Interactive terminals
update that status in place; redirected output keeps plain progress lines.
Cached tools resolve immediately without a download progress stream.

VPM's tool resolver is generic: the resolver consumes catalog records keyed by
tool name, exact version and supported host target. Tailwind release URLs and
official release SHA-256 values are catalog data, not Tailwind-specific logic
inside the resolver. A tool/version/target cache entry is accepted only after
its trusted digest matches; partial or corrupt downloads are removed and
retried. VPM cache is shared between projects using the same tuple.

## Build behavior

For `vutnet run` and `vutnet build`, Tailwind compiles to a temporary CSS
candidate. Vutnet then builds the application. Only after both commands succeed
is the candidate copied atomically to the configured CSS output. A failure
leaves the previous CSS output intact. No section means no Tailwind executable
lookup or CSS build.

The Tailwind command receives input resolved from the project root and a private
candidate-output path, and runs with the project root as its working directory. Vutnet does not
rewrite CSS, parse Tailwind directives, or scan class names itself. Projects
whose Vut source files are not included by Tailwind's automatic source
detection must add an explicit `@source` directive in their CSS entry; the
standalone CLI remains responsible for class extraction and CSS dependency
resolution.

## Development watch and candidate transaction

`vutnet dev` resolves the executable and reads `vutnet.toml` once when the
supervisor starts. For each debounced source revision, one build transaction
creates a unique generation directory under `.vut/dev/`:

```text
.vut/dev/generation-N/
├── app[.exe]
└── assets/app.css
```

The existing VPM application build produces the normal profile artifact, which
Vutnet snapshots into that generation; the one-shot Tailwind build writes its
CSS candidate there. The Tailwind CLI is not run with `--watch`: its watch
process would write independently of the Vutnet app-build result and would
break the last-known-good transaction. Instead, Vutnet's existing debounced
watcher observes `.vut`, CSS input/import files in its watched project and
local dependency roots, and the VPM manifest/lock. `.css` is added to the
watcher's accepted source extensions, and the configured output plus
`.vut/` generated data are excluded to prevent self-trigger loops. The config is loaded at
supervisor startup; changes to `vutnet.toml` are not hot-reloaded, so restart
`vutnet dev` to apply them because the resolved tool version is fixed for that
session. Tailwind itself
continues to resolve CSS imports and discover class candidates on every
one-shot build; Vutnet does not maintain a second CSS dependency graph.

The Vut application build and CSS build must both succeed before replacement
begins. A build failure deletes only that candidate and leaves the active
server and its CSS output untouched. On success, the supervisor stops and
reaps the old server, publishes the candidate CSS atomically, then starts the
candidate application. The previous application artifact and CSS are retained
until the new child has started; if launch fails, Vutnet restores the previous
CSS and attempts to restart the last-known-good app before waiting for another
change. There is no request served during the short stop/publish/restart window,
so clients cannot observe an old app paired with newly published CSS.

On initial application or CSS build failure, `vutnet dev` logs the error and
remains alive with its watcher active. Subsequent application or Tailwind
failures retain the current successful generation. A later successful
transaction recovers and replaces it. Tool resolution failure is a startup
configuration/tooling error; it is reported before the watcher starts.

This remains a persistent supervisor with one active candidate transaction.
Changes arriving during a build coalesce into a subsequent build of the newest
source state. Tailwind build errors use the existing dev logger; CSS diagnostics
are included without terminal clearing or ANSI output from process logic.

## V1 support and tests

V1 resolves exact catalogued versions for supported host targets. A requested
version or target without trusted catalog metadata is an actionable error; no
network fallback to `latest` occurs. V1 does not provide plugin configuration,
configurable Tailwind flags, a Node-based fallback, or an independent Tailwind
watcher.

Focused tests cover strict config parsing and path resolution; absent/present
Tailwind config; VPM tool target selection, checksum validation, shared cache
reuse, corrupt-cache recovery, interrupted downloads and concurrent resolution;
command argument construction; app-build and CSS-build failures; initial failure
recovery; candidate generation publication; atomic output replacement;
last-known-good application/CSS retention and rollback after child launch
failure; and `.css`/config watcher events without generated-output loops. VPM
tool-resolution tests use a local HTTP fixture. Release qualification should
additionally smoke-test the pinned standalone CLI on each supported host target;
unit tests do not download that large binary.

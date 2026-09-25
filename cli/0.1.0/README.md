# cli

`cli` is a declarative, type-safe command-line framework for
[Vut](https://github.com/duongonix/vut), distributed as a normal
[VPM](https://github.com/duongonix/vpm) package.

```vut
import cli
```

It is **not** part of the Vut builtins, standard library, compiler, or runtime.
It is an ordinary third-party package written in pure Vut, with a native-first
runtime and a pure (I/O-free) parsing/help core.

## Install

```bash
vpm add cli
```

## Quick start

```vut
import cli

fn serve(ctx: cli.Context) -> int:
  path = ctx.arg_or("path", ".")
  port = ctx.option_int_or("port", 3000)
  verbose = ctx.flag("verbose")

  out("Serving $path on port $port")
  if verbose:
    out("Verbose mode")
  0

fn main() -> int:
  serve_cmd = (
    cli.command("serve", about = "Start server")
      .alias("s")
      .arg("path", about = "Directory to serve", default_value = ".")
      .option("port", short = "p", about = "Port", default_value = "3000")
      .flag("verbose", short = "v", about = "Verbose output")
      .handle(serve)
  )

  app = (
    cli.app("demo", version = "0.1.0", about = "Example CLI")
      .command(serve_cmd)
  )

  cli.run(app)
```

```bash
demo serve ./public --port 8080 -v
demo --help
demo --version
demo serve --help
```

## API reference

### Types

| Type | Meaning |
| --- | --- |
| `App` | application metadata and its commands |
| `Command` | a subcommand with arguments, options, flags, aliases, and a handler |
| `Arg` | a positional argument specification |
| `Option` | a named value (`--port`) |
| `Flag` | a boolean switch (`--verbose`) |
| `Context` | parsed values handed to a handler |
| `Invocation` | resolved command plus its `Context` |
| `Token`, `TokenKind` | syntactic argv tokens |
| `Error`, `ErrorKind` | parse/validation errors |

### Constructors

```vut
cli.app(name: str, version: str = "", about: str = "") -> App
cli.command(name: str, about: str = "") -> Command
cli.arg(name: str, about: str = "", required: bool = false, default_value: str = "") -> Arg
cli.option(name: str, short: str = "", about: str = "", required: bool = false, default_value: str = "", hidden: bool = false) -> Option
cli.flag(name: str, short: str = "", about: str = "", hidden: bool = false) -> Flag
cli.context(command: str = "") -> Context
```

### Command builders (functional)

Every builder returns a **new** `Command` and never mutates the receiver:

```vut
Command.about(text: str) -> Command
Command.alias(name: str) -> Command
Command.hidden(value: bool = true) -> Command
Command.arg(name: str, about: str = "", required: bool = false, default_value: str = "") -> Command
Command.option(name: str, short: str = "", about: str = "", required: bool = false, default_value: str = "", hidden: bool = false) -> Command
Command.flag(name: str, short: str = "", about: str = "", hidden: bool = false) -> Command
Command.handle(handler: fn(Context) -> int) -> Command
```

`App.command(cmd) -> App` adds a command and likewise returns a new `App`.

### Context accessors

```vut
Context.arg_or(name: str, fallback: str) -> str
Context.option_or(name: str, fallback: str) -> str
Context.flag(name: str) -> bool
Context.option_int(name: str) -> int?
Context.option_int_or(name: str, fallback: int) -> int
Context.arg_int(name: str) -> int?
Context.arg_int_or(name: str, fallback: int) -> int
```

`option_int`/`arg_int` validate the text first and return `null` for
non-integers, so invalid input never silently becomes `0`. Functional setters
`with_arg`/`with_option`/`with_flag` exist for building a `Context` directly.

### Tokenizer and parser (pure)

```vut
cli.tokenize(args: list[str]) -> list[Token]
cli.parse(app: App, args: list[str]) -> result[Invocation, Error]
```

`parse` resolves the subcommand (by name or alias), binds positionals, options,
and flags, applies defaults, checks required values, and returns an `Error`
otherwise. It performs no I/O.

### Help and version (pure)

```vut
cli.help_text(app: App) -> str
cli.command_help_text(app: App, name: str) -> str
cli.version_text(app: App) -> str
```

### Runner

```vut
cli.run(app: App) -> int              # reads process argv (drops argv[0])
cli.run_with(app: App, args: list[str]) -> int
```

`run_with`/`run` validate the app, intercept `--help`/`-h` and `--version`/`-V`
before `--`, dispatch the matched handler, and return the process exit code.

```vut
cli.EXIT_OK = 0
cli.EXIT_USAGE = 2
```

### Validation

```vut
cli.validate(app: App) -> result[unit, Error]
```

Rejects reserved command/option/flag names (`help`, `version`) and shorts
(`-h`, `-V`), duplicate option/flag/argument names, duplicate shorts, and a
short shared by an option and a flag. `run`/`run_with` call this automatically.

## Behavior

### Argument syntax

| Input | Meaning |
| --- | --- |
| `serve ./public` | positional argument |
| `--port 8080` | long option with a separate value |
| `--port=8080` | long option with an attached value |
| `-p 8080` | short option with a separate value |
| `-p8080` | short option with an attached value |
| `--verbose`, `-v` | boolean flag |
| `-vd` | combined short flags |
| `-vp8080` | flag followed by an option with an attached value |
| `--` | everything after it is a positional |

Duplicate options use the last value. Unknown options, missing values, missing
required arguments, and extra positionals produce typed errors.

### Reserved and hidden

`help`, `version`, `-h`, and `-V` are reserved and rejected by validation.
Commands, options, and flags marked `hidden` are omitted from help but still
work at runtime.

### Suggestions

An unknown command reports the closest command or alias (edit distance ≤ 2):

```text
error: unknown command `serv`; did you mean `serve`?
```

### Exit codes

```text
0   success (including --help and --version)
2   usage error (parse or validation failure)
N   the handler's own return value
```

### Builder semantics

Builders are functional and use value semantics:

```vut
base = cli.command("serve")

a = base.arg("path")
b = base.arg("host")

# base.args == []  a.args == ["path"]  b.args == ["host"]
```

Each builder copies only the field it changes, mutates that copy, and
constructs a new aggregate. Multiline chains must be wrapped in parentheses,
following Vut's delimiter-aware newline rules:

```vut
cmd = (
  cli.command("serve")
    .arg("path")
    .flag("verbose", short = "v")
)
```

## Dogfood example

`examples/hello_cli.vut` is a small real CLI (`init`, `build`, `run`, `config`,
with aliases, options, flags, typed port parsing, and filesystem output). Run it
against the package with:

```powershell
./examples/run.ps1 -Vpm <path-to-vpm>
```

The script mounts the package, runs a matrix of real invocations, and asserts
stdout and exit codes.

## Tests

Integration tests live in `dev-tests/` and run through a scratch-project harness
that mounts the package as `src/cli/` and exercises the public `import cli`
surface:

```powershell
./dev-tests/run.ps1 -Vpm <path-to-vpm>
```

Coverage: constructors, functional fluent chains, Command/App COW semantics,
nested managed values, named and capturing handlers, a 20-step chain, the pure
tokenizer, the argument binder (positionals, long/short options, `--x=v`,
`-x v`, `-xv`, clusters, aliases, defaults, `--`, and every error case), help /
usage / version rendering, the run adapter, typed accessors, hidden specs,
validation, and command suggestions.

## Limitations

* Native targets are the primary V1 target. The pure core compiles for
  `wasm32-wasip1`, but the full package (the `run` adapter's stdlib `io`/`env`
  use, and the example's filesystem use) currently hits an unsupported
  `ResourceDeref` instruction in the WASM backend.
* Multiline fluent chains require wrapping parentheses (a Vut newline rule).
* Options and flags are string/boolean; typed conversion is explicit through the
  `Context` accessors.

## License

MIT

# Keel CLI reference

The current source tree builds `keel` and `keelc` from the same driver. `keel`
is the user-facing command; `keelc` exists for the conformance harness and
currently accepts the same arguments.

## Synopsis

```text
keel <build|run|fmt|test|check|audit> <file.keel> [--milestone M<N>]
keel gen <schema.proto>
```

The CLI does not yet implement `--help`, `--version`, response files, global
configuration, or shell completion. Missing/invalid arguments print usage to
stderr and exit 2.

## Development milestone gate

```text
--milestone M<N>
```

The compiler currently defaults to M1. Use `--milestone M7` for all implemented
language and toolchain behavior. This flag is development scaffolding, not a
promised post-1.0 user interface.

## `keel check`

```sh
keel check main.keel --milestone M7
```

Reads the source and adjacent workspace manifest, then runs parsing, name
resolution, and typechecking. It emits diagnostics to stderr and produces no
artifact.

Diagnostics are sorted by source span and stable code. A typical diagnostic is:

```text
error[K0303]: cannot assign to immutable binding `x`
  --> main.keel:3:5
```

## `keel run`

```sh
keel run main.keel --milestone M7
```

Checks the program, lowers it through KIR, emits temporary Go source, and invokes
`go run`. Program stdout/stderr and success status are forwarded. The temporary
directory is removed when the command exits. Program stdin is currently closed.

## `keel build`

```sh
keel build main.keel --milestone M7
```

Checks and lowers the program, then invokes:

```text
go build -trimpath -buildvcs=false -o <artifact>
```

The artifact is written beside the input and named after its file stem
(`service.keel` produces `service`, plus the platform executable suffix).

The reproducibility conformance case verifies byte-identical output for the
same Core input. Programs importing `std.sql` currently cause the driver to
create a Go module and run `go mod tidy` for `modernc.org/sqlite`; that operation
may access the Go module proxy. Therefore the no-network build guarantee is not
yet true for every implemented standard-library program.

## `keel fmt`

```sh
keel fmt main.keel --milestone M7
```

Parses the input and writes canonical source to stdout. It does not modify the
file and it does not run semantic typechecking. Parse diagnostics go to stderr.

Do not redirect output directly onto the input because the shell truncates the
file before `keel` reads it. Use a temporary file:

```sh
keel fmt main.keel --milestone M7 > main.keel.formatted
mv main.keel.formatted main.keel
```

## `keel test`

```sh
keel test service_test.keel --milestone M7
```

Checks the source, discovers every `test "name" { ... }` block, emits a temporary
Go test harness, and runs it. Passing tests print one line:

```text
test "addition holds" ... ok
```

An assertion failure exits nonzero and includes its source line.

## `keel audit`

```sh
keel audit src/main.keel --milestone M7
```

The file locates the package/workspace to audit. The command reads `keel.toml`,
resolves local path dependencies, and prints the deterministic effective
capability report defined by spec chapter 11. It does not build or run the
program.

Example shape:

```text
users_service 0.1.0
  net: self, http_client 0.1.0
  (fs, exec, env, ffi, unsafe-memory: not present)
```

## `keel gen`

```sh
keel gen schema.proto
```

Reads a protobuf schema and writes canonical Keel source to stdout. Only the
chapter-17 proto3 message/enum subset is implemented. Malformed schemas produce
`K1601`; well-formed unsupported constructs produce `K1602`. Other extensions
are usage errors and exit 2.

Generation is explicit and never runs as part of `keel build`.

## Exit status

| Status | Meaning |
|---|---|
| `0` | command and compiled program/tests succeeded |
| `1` | compiler diagnostic, lowering/backend failure, generated program failure, or test failure |
| `2` | invalid CLI usage, unreadable input, unsupported schema extension, or failure to invoke required host tooling |

## Not implemented

`keel lint`, `keel fix`, `keel lsp`, `keel init`, package publishing, and OCI
image output are roadmap work, not hidden commands.

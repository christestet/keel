# Keel CLI reference

The current source tree builds `keel` and `keelc` from the same driver. `keel`
is the user-facing command; `keelc` exists for the conformance harness and
currently accepts the same arguments.

## Synopsis

```text
keel <build|run|fmt|test|check|audit> <file.keel> [--milestone M<N>]
keel gen <schema.proto>
keel lsp
keel --version
```

`keel --version` (also `-V`) prints one line — `keel 0.1.0 (commit <hash>)` —
and exits 0. The commit is embedded from `KEEL_BUILD_COMMIT` at build time by
the release build; source builds report `commit unknown`. `keelc --version`
reports as `keelc`.

The CLI does not yet implement `--help`, response files, global
configuration, or shell completion. Missing/invalid arguments print usage to
stderr and exit 2.

## Development milestone gate

```text
--milestone M<N>
```

The compiler defaults to the latest implemented milestone (M7) — the complete
current language. Pass `--milestone M<N>` only to check a program against an
earlier milestone's gate. This flag is development scaffolding, not a
promised post-1.0 user interface.

## `keel check`

```sh
keel check main.keel
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
keel run main.keel
```

Checks the program, lowers it through KIR, emits temporary Go source, and invokes
`go run`. Program stdout/stderr and success status are forwarded. The temporary
directory is removed when the command exits. Program stdin is currently closed.

## `keel build`

```sh
keel build main.keel
```

Checks and lowers the program, then invokes:

```text
go build -trimpath -buildvcs=false -o <artifact>
```

The artifact is written beside the input and named after its file stem
(`service.keel` produces `service`, plus the platform executable suffix).

A clean, diagnostic-free build also writes a hidden stamp beside the artifact
(`.service.keelstamp`) recording the build inputs (source, compiler, Go
toolchain, milestone, artifact hash). Re-running `keel build` with identical
inputs verifies the stamp and the artifact's contents, then exits without
recompiling — delete the stamp (or the artifact) to force a full rebuild. A
build that emits any diagnostic never writes a stamp, so warnings are
reprinted on every build.

The reproducibility conformance case verifies byte-identical output for the
same Core input. Programs importing `std.sql` currently cause the driver to
create a Go module and run `go mod tidy` for `modernc.org/sqlite`; that operation
may access the Go module proxy. Therefore the no-network build guarantee is not
yet true for every implemented standard-library program.

## `keel fmt`

```sh
keel fmt main.keel
```

Parses the input and writes canonical source to stdout. It does not modify the
file and it does not run semantic typechecking. Parse diagnostics go to stderr.

Do not redirect output directly onto the input because the shell truncates the
file before `keel` reads it. Use a temporary file:

```sh
keel fmt main.keel > main.keel.formatted
mv main.keel.formatted main.keel
```

## `keel test`

```sh
keel test service_test.keel
```

Checks the source, discovers every `test "name" { ... }` block, emits a temporary
Go test harness, and runs it. Passing tests print one line:

```text
test "addition holds" ... ok
```

An assertion failure exits nonzero and includes its source line.

## `keel audit`

```sh
keel audit src/main.keel
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

## `keel lsp`

```sh
keel lsp
```

Runs the M8 base LSP server (spec chapter 16, KDR-0103) over stdio until it
receives `exit`. It takes no file argument — documents are opened, changed,
and closed through `textDocument/didOpen`/`didChange`/`didClose`
notifications, not CLI arguments. Every document is checked at the highest
implemented Core milestone (M7), independent of the `--milestone` flag, which
applies only to file commands.

Advertised capabilities: incremental text sync, diagnostics, go-to-definition,
hover, completion, and document symbols. Definition/hover/completion/document
symbols resolve module-level `fn`/`struct` declarations and a small built-in
table by name; there is no local-scope (parameter/`let`-binding) resolution
yet. References, formatting, code actions, workspace symbols, rename, and
inlay hints are not advertised — see spec chapter 16 §16.1.

Malformed JSON-RPC input and unsupported methods produce JSON-RPC error
responses; they do not terminate the server. All ten behaviors above are
locked by golden transcripts in [`tests/lsp/m8-base`](../tests/lsp/m8-base),
replayed against the real server in `compiler/keelc-lsp/tests/transcripts.rs`.

## Exit status

| Status | Meaning |
|---|---|
| `0` | command and compiled program/tests succeeded |
| `1` | compiler diagnostic, lowering/backend failure, generated program failure, or test failure |
| `2` | invalid CLI usage, unreadable input, unsupported schema extension, or failure to invoke required host tooling |

## Not implemented

`keel lint`, `keel fix`, `keel init`, package publishing, and OCI image output
are roadmap work, not hidden commands.

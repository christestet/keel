# Troubleshooting

This guide covers the current source-built M7 toolchain. Commands below assume
`target/release/keel`; substitute another binary path if needed.

## The command only prints usage

The CLI has no `--help` or `--version` yet. It requires a command and input path:

```sh
target/release/keel check main.keel
```

The milestone value must use an uppercase `M` followed by a number. Invalid or
missing arguments exit 2.

## A landed feature is reported as not in Core

The development CLI defaults to M1. Interfaces, generics, structured
concurrency, arenas, packages, and later standard-library behavior require the
appropriate milestone gate. Use M7 for the full implemented surface:

```sh
target/release/keel check main.keel
```

`K0905` for `extern` is different: C FFI is genuinely not implemented at M7.

## `keel fmt` did not change the file

Formatting is written to stdout. It never edits the input:

```sh
target/release/keel fmt main.keel > main.keel.formatted
mv main.keel.formatted main.keel
```

Never redirect directly to `main.keel`; the shell truncates it before the
formatter reads it.

## The Go toolchain cannot be invoked

`run`, `test`, and `build` currently require `go` on `PATH`. Confirm:

```sh
go version
cargo build --release -p keelc-driver
```

`check`, `fmt`, `audit`, and `gen` do not require Go after the Keel binary has
been built.

## SQL builds cannot resolve `modernc.org/sqlite`

`std.sql` causes the driver to create a temporary Go module and run
`go mod tidy`. The first build needs a populated Go module cache or access to
the configured module proxy. Diagnose with:

```sh
go env GOPROXY GOMODCACHE
```

If policy forbids network access, pre-populate the approved module cache outside
the Keel build. The current compiler has no vendoring/offline flag.

## A PostgreSQL or MySQL connection reports an unknown driver

The runtime recognizes `postgres://`, `postgresql://`, and `mysql://` prefixes,
but only the SQLite Go driver is bundled. Use SQLite with the current toolchain.
Do not add a driver dependency as an undocumented workaround; backend dependency
changes require their own decision and review.

## Package imports validate but symbols do not link

M7 implements manifest parsing, path-dependency graph validation, `use`-path
declaration checks, and capability rollup. It does not compile dependency source
or resolve dependency symbols into the root module. A declared `use helper.x`
can pass package validation while calls into that package remain unsupported.

Keep code in one source module for executable behavior until cross-package
resolution has conformance coverage.

## Capability checks appear to be missing

Files without adjacent `keel.toml` are implicit packages, and the current driver
skips capability enforcement for them. Add an explicit manifest before relying
on `keel audit` or capability diagnostics.

For explicit packages:

- `std.http` needs `net`;
- `std.sql` needs `net` and `fs`;
- `std.config` needs `env`;
- dependency capability declarations must be repeated by dependents.

Run:

```sh
target/release/keel audit main.keel
```

## `config.load<T>()` cannot find a value

Field names map mechanically to upper snake case. For example,
`database_url` reads `DATABASE_URL`. Required `Secret` fields produce
`MissingSecret`; other required fields produce `MissingEnvVar`. A declaration
default is used when its environment variable is absent.

## The HTTP server will not start

Check that the port is between 1 and 65535 and not already bound. `http.serve`
returns `http.BindFailed` for bind errors. The current runtime does not configure
TLS or retry another port.

## `keel gen` rejects a valid protobuf file

Only the chapter-17 proto3 data subset is implemented: top-level messages,
enums, supported scalar fields, named fields, and `repeated`. Services/RPCs,
`bytes`, maps, `oneof`, optional/required labels, options, and nested messages
produce `K1602` rather than an approximate mapping. OpenAPI is M10 work.

## A diagnostic or output seems wrong

Look up the code in [Compiler diagnostics](diagnostics.md) and find the smallest
matching case under `tests/conformance/`. The suite is the executable language
definition.

Report a compiler defect when:

- malformed user input panics or terminates without a diagnostic;
- the stable code conflicts with the relevant conformance case;
- diagnostic or generated output changes between identical runs;
- the formatter is not idempotent;
- a documented example fails under its stated milestone.

Include the compiler commit, host OS, Rust/Go versions, exact command, minimal
source, and complete stderr. If normative prose and a conformance case disagree,
file an issue rather than editing either to make the compiler pass.

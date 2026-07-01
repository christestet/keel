# Getting started with Keel

Keel is pre-1.0. The current toolchain is built from this repository and uses
Go as its backend. There are no published installers or release binaries yet.

## Prerequisites

- a current stable Rust toolchain with Cargo;
- Go 1.21 or newer;
- this repository checked out locally.

Programs using `std.sql` also resolve the `modernc.org/sqlite` Go module. The
first such build may need network access through the Go module proxy.

## Build the toolchain

From the repository root:

```sh
cargo build --release -p keelc-driver
```

This produces:

- `target/release/keel`, the user-facing command;
- `target/release/keelc`, the conformance-runner command.

## Run the hello-world example

```sh
target/release/keel run examples/hello.keel --milestone M7
```

Output:

```text
hello, keel
```

The development CLI currently defaults to the M1 parser gate. Pass
`--milestone M7` to enable the complete implemented language.

## Write a program

Create `hello.keel`:

```keel
fn main() {
    let name = "Keel"
    print("hello, {name}")
}
```

Check and run it:

```sh
target/release/keel check hello.keel --milestone M7
target/release/keel run hello.keel --milestone M7
```

`check` parses, resolves, and typechecks without generating an executable.
Compiler errors use stable `K####` codes and point to the primary source span.

## Format source

`keel fmt` prints canonical source to stdout; it does not modify the input:

```sh
target/release/keel fmt hello.keel --milestone M7
```

To replace a file safely, write to a temporary file first:

```sh
target/release/keel fmt hello.keel --milestone M7 > hello.keel.formatted
mv hello.keel.formatted hello.keel
```

## Build an executable

```sh
target/release/keel build hello.keel --milestone M7
./hello
```

`build` writes the executable beside the source file. The current backend emits
Go and invokes the Go toolchain with reproducible-build flags.

## Write a test

Create `math_test.keel`:

```keel
test "addition holds" {
    assert 1 + 1 == 2
}
```

Run it:

```sh
target/release/keel test math_test.keel --milestone M7
```

Output:

```text
test "addition holds" ... ok
```

## Current boundaries

- Package dependencies are local paths; there is no package registry.
- The Go toolchain remains a build dependency until the M11 native backend.
- `keel lsp` implements the M8 base capabilities (diagnostics, definition,
  hover, completion, document symbols) at module scope only — see
  [CLI reference](cli-reference.md#keel-lsp). `keel lint` and `keel fix` are
  not implemented.
- C FFI and OpenAPI generation are planned for M10.

Continue with the [language tour](language-tour.md), then use the
[specification](spec/keel-core.md) when exact semantics matter.

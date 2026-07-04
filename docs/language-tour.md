{% raw %}
# A tour of Keel

This tour describes the language implemented through M7. The normative sources
remain the [specification](spec/) and the
[conformance suite](../tests/conformance/). When this tour and a conformance
case disagree, the case wins.

Run examples with the complete development gate:

```sh
target/release/keel run example.keel --milestone M7
```

## Source files

Keel source is UTF-8. Statements end at newlines; semicolons are rejected.
Comments run from `//` to the end of the line.

Type names use `UpperCamelCase`. Functions, values, fields, and modules use
`snake_case`.

```keel
fn main() {
    // Local inference is allowed.
    let greeting = "hello"
    print(greeting)
}
```

## Values and types

Primitive types are `Int`, `Float`, `Bool`, `String`, `Char`, and `Unit`.
`Option<T>`, `Result<T, E>`, `List<T>`, and `Map<K, V>` are built-in generic
types.

Keel has no `null` and performs no implicit numeric conversions:

```keel
let count: Int = 2
let ratio: Float = Float.from(count) / 4.0
let label: Option<String> = None
```

String interpolation uses braces. Double braces produce literal braces:

```keel
let name = "Ada"
print("hello, {name}")
print("{{literal braces}}")
```

## Bindings and functions

`let` bindings are immutable. Use `mut` only when reassignment is required.

```keel
fn increment(value: Int) -> Int {
    value + 1
}

fn main() {
    let fixed = 1
    mut changing = increment(fixed)
    changing = increment(changing)
    print("{changing}")
}
```

Parameter and return types are explicit. `-> Unit` may be omitted. A trailing
parameter may have a default:

```keel
fn page_size(limit: Int = 50) -> Int {
    limit
}

fn main() {
    print("{page_size()}")
    print("{page_size(10)}")
}
```

Blocks and `if` are expressions. A block's last expression is its value:

```keel
fn classify(n: Int) -> String {
    if n > 0 { "positive" } else { "non-positive" }
}
```

Keel also provides `while`, `break`, `continue`, and explicit `return`.

## Structs

Struct construction must provide every field unless the declaration supplies a
default. `Option<T>` fields do not silently default to `None`.

```keel
struct User {
    name: String
    email: Option<String>
    active: Bool = true
}

fn main() {
    let user = User {
        name: "Ada"
        email: None
    }
    print(user.name)
}
```

## Enums and exhaustive matching

Enum variants may carry values. `match` must cover every variant:

```keel
enum Status {
    Active,
    Suspended(reason: String),
    Deleted,
}

fn describe(status: Status) -> String {
    match status {
        Active            => "active",
        Suspended(reason) => "suspended: {reason}",
        Deleted           => "deleted",
    }
}
```

Patterns can nest, bind payloads, and use guards:

```keel
fn sign(value: Option<Int>) -> String {
    match value {
        Some(n) if n > 0 => "positive",
        Some(n)          => "non-positive",
        None             => "missing",
    }
}
```

A wildcard arm is allowed, but matching a same-module enum with `_` produces a
warning because a new variant would otherwise be hidden.

## Recoverable errors

Functions return `Result<T, E>` for recoverable failures. `?` extracts `Ok` or
returns the error from the enclosing function:

```keel
fn positive(n: Int) -> Result<Int, String> {
    if n > 0 { Ok(n) } else { Err("not positive") }
}

fn doubled(n: Int) -> Result<Int, String> {
    let value = positive(n)?
    Ok(value * 2)
}
```

`catch` handles errors where an expression is used:

```keel
enum LookupError {
    NotFound,
    Unavailable,
}

fn lookup() -> Result<String, LookupError> {
    Err(NotFound)
}

fn display() -> String {
    let value = lookup() catch err {
        NotFound   => return "missing",
        Unavailable => return "unavailable",
    }
    value
}
```

Function signatures may use union error types such as
`Result<User, ParseError | DbError>`. `Error` is the universal opaque boundary
error: it can absorb other errors but cannot be destructured.

`panic("message")` is reserved for unrecoverable bugs. Panics are not catchable.

## Interfaces

Interfaces are nominal, contain at most five methods, and have explicit `impl`
blocks. There are no default methods or inheritance.

```keel
interface Stringer {
    fn to_string(self) -> String
}

struct Point {
    x: Int
    y: Int
}

impl Stringer for Point {
    fn to_string(self) -> String {
        "({self.x}, {self.y})"
    }
}

fn main() {
    let point = Point { x: 1, y: 2 }
    let value: Stringer = point
    print(value.to_string())
}
```

## Constrained generics

Every user-defined type parameter requires an interface bound:

```keel
fn render<T: Stringer>(value: T) -> String {
    value.to_string()
}
```

The compiler checks method use at the generic definition and bound satisfaction
at each call site. Generics are currently erased to their interface bound by the
Go backend; they are not monomorphized.

## Structured concurrency

Concurrent work lives inside `scope`; `spawn` is invalid outside one. The scope
joins all tasks before yielding its value, and task handles cannot escape.

```keel
fn work() -> Int {
    41
}

fn main() {
    let value = scope {
        let task = spawn work()
        task.value + 1
    }
    print("{value}")
}
```

Scopes are fail-fast for task errors and choose the first error by spawn order.
Deadlines use `time.Duration`:

```keel
use std.time

fn main() -> Result<Unit, Cancelled> {
    scope(deadline: time.seconds(1)) {
        check_cancel()?
    }
    Ok(())
}
```

Keel deliberately has no `async`/`await` and no detached tasks.

## Arenas

An `arena` is a lexical allocation region. Region-backed values may not escape
the block:

```keel
struct Pair {
    left: Int
    right: Int
}

fn total() -> Int {
    arena {
        let pair = Pair { left: 20, right: 22 }
        pair.left + pair.right
    }
}
```

Current M7 ceiling: the Go backend lowers `arena` to an ordinary block and the
compiler enforces the conformance-backed tail escape rule. Real region
allocation and complete escape analysis are part of the M11 native backend.

## Tests

Tests are ordinary source blocks discovered by `keel test`:

```keel
test "addition holds" {
    assert 1 + 1 == 2
}
```

The formatter has one canonical style and no configuration options.

## Packages and capabilities

A directory containing `keel.toml` is a package. Dependencies are currently
local paths. Packages declare authority such as `net` and `fs`; the compiler
checks declarations transitively and `keel audit` reports the effective set.

See the normative [module/package specification](spec/06-modules-packages.md)
and [capability specification](spec/11-capabilities.md).

## Deliberately absent or unfinished

- no inheritance, macros, reflection, exceptions, operator overloading, or
  `async`/`await`;
- no C FFI yet;
- no package registry;
- no LSP server yet;
- no native backend yet.

These boundaries are design choices or scheduled work, not invitations for the
parser to accept unratified syntax. Consult the [roadmap](../ROADMAP.md) and
[KDR index](kdr/INDEX.md) before proposing language surface.
{% endraw %}

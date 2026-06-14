# 08 — Generics

This chapter is **normative**. It adds **user-defined generic functions and
types** to the language, the parametric-polymorphism mechanism recorded in
[`KDR-0022`](../kdr/0022-interface-constrained-generics.md). It does not restate
the frozen rules in [`keel-core.md`](keel-core.md); on any conflict with
`keel-core.md`, file an issue rather than reconciling silently (the prime
directive in the root [`AGENTS.md`](../../AGENTS.md)).

Implementation status: not yet implemented; see
[`docs/milestone-status.md`](../milestone-status.md) §M5.

Generics in Keel are structurally constrained by interfaces:

- Every type parameter must be bounded by an explicit interface.
- Constraint satisfaction is **structural**: a type satisfies an interface bound
  if it has methods matching every signature declared in the interface. No
  `impl` block is required for constraint satisfaction.
- Runtime interface dispatch remains **nominal**: an explicit `impl Interface
  for Type` block is required when a value is used as an interface type (see
  §7.2 of the interfaces spec). The two paths are independent.
- No union type constraints, no type-set algebra, no `|` operator in constraint
  position. Constraints are simple method sets.

## 8.1 Generic function declarations

A generic function declares type parameters in angle brackets before its
parameter list:

```keel
fn identity[T: Stringer](value: T) -> T {
    value
}
```

A type parameter is in scope for the entire function signature and body. The
body may only call methods declared in the interface bound; calling any other
method is a compile error (`K0802`).

Multiple type parameters are separated by commas:

```keel
fn transform[A: Stringer, B: Stringer](input: A, func: fn(A) -> B) -> B {
    func(input)
}
```

Each type parameter must have its own interface bound. A generic function with
an unbound type parameter is ill-formed (`K0801`).

## 8.2 Generic type declarations

A generic struct or enum declares type parameters in angle brackets after the
type name:

```keel
struct Pair[A: Stringer, B: Stringer] {
    first: A
    second: B
}

enum Result[T: Stringer, E: Stringer] {
    Ok(value: T)
    Err(reason: E)
}
```

Type parameters are in scope for all field types and method signatures within
an `impl` block for the type. Methods on a generic type may use the type's
type parameters without redeclaring them:

```keel
impl Stringer for Pair[A, B] {
    fn to_string(self) -> String {
        "{self.first}, {self.second}"
    }
}
```

## 8.3 Constraint satisfaction

Constraint satisfaction is **structural**. A concrete type `T` satisfies an
interface bound `I` at a generic instantiation site iff for every method
declared in `I`, `T` has a method with the same name, the same parameter
count, the same parameter types, and the same return type.

A method is considered "available on `T`" if there exists an `impl X for T`
block (for any interface `X`) that declares a method matching the signature.
The name of the interface defining the impl does not matter; only the method
signatures are compared.

```keel
interface HasArea {
    fn area(self) -> Float
}

interface HasPerimeter {
    fn perimeter(self) -> Float
}

struct Circle {
    radius: Float
}

impl HasPerimeter for Circle {
    fn perimeter(self) -> Float {
        2.0 * 3.14159 * self.radius
    }
}

// HasPerimeter.impl provides `perimeter(self) -> Float` for Circle.
// The structural check for HasArea looks for `area(self) -> Float`.
// Circle does NOT satisfy HasArea — no impl provides `area`.
// fn print_area[T: HasArea](shape: T) { ... }  // K0803 if called with Circle

impl HasArea for Circle {
    fn area(self) -> Float {
        3.14159 * self.radius * self.radius
    }
}

// Now Circle satisfies HasArea structurally.
fn print_area[T: HasArea](shape: T) {
    print(shape.area())
}
```

A type argument that does not satisfy its bound is a compile error (`K0803`).
The diagnostic lists the missing or mismatched methods.

The same `impl` block serves both roles at once: it provides the method for
runtime dispatch (interface values, §7.2) and supplies the method evidence
for structural constraint checking (generics). No separate declaration is
needed for each role.

### 8.3.1 Intrinsic satisfaction for primitive types

The built-in primitive types (`Int`, `Float`, `Bool`, `String`, `Char`) satisfy
interface bounds structurally if they have compiler-built-in methods matching
the interface. No `impl` block is needed or possible.

## 8.4 Instantiation

A generic function or type is instantiated by providing concrete type arguments
in angle brackets:

```keel
let c = Circle { radius: 2.5 }
print_area[Circle](c)

let p = Pair[Circle, Circle] {
    first: c,
    second: c,
}
```

When type arguments can be inferred from the value arguments, they may be
omitted:

```keel
print_area(c)  // T inferred as Circle
```

Type inference is limited to the parameters of the call; return type
position alone does not drive inference. Ambiguous inference is a compile
error.

## 8.5 Examples (normative, extracted by CI)

```keel
interface HasArea {
    fn area(self) -> Float
}

struct Circle {
    radius: Float
}

struct Rectangle {
    width: Float
    height: Float
}

impl HasArea for Circle {
    fn area(self) -> Float {
        3.14159 * self.radius * self.radius
    }
}

impl HasArea for Rectangle {
    fn area(self) -> Float {
        self.width * self.height
    }
}

fn print_area[T: HasArea](shape: T) {
    print(shape.area())
}

fn main() -> Unit {
    let c = Circle { radius: 2.0 }
    let r = Rectangle { width: 3.0, height: 4.0 }
    print_area(c)  // prints 12.56636
    print_area(r)  // prints 12
}
```

## 8.6 Error conditions

The following are errors with stable `K####` codes:

- **`K0801` — type parameter without interface bound.** Every type parameter
  must have an explicit interface constraint. Unconstrained `[T]` is rejected.

- **`K0802` — method not in interface bound.** A generic function body calls a
  method on a value of type parameter type, but that method is not declared in
  the type parameter's interface bound. This check fires at the generic
  definition site.

- **`K0803` — type argument does not satisfy interface bound.** A concrete type
  provided at an instantiation site does not structurally match the interface
  bound. The diagnostic identifies the missing or mismatched methods.

- **`K0804` — duplicate type parameter name.** The same identifier appears more
  than once in a single type parameter list.

- **`K0805` — type parameter name shadows existing type.** A type parameter
  name collides with a type, interface, or enum declared in the same module.

- **`K0806` — too many type parameters.** A generic type or function declares
  more type parameters than the implementation limit (256).

- **`K0807` — interface used as generic constraint declares more than five
  methods.** Interface bounds on type parameters inherit the five-method limit
  from KDR-0003. In practice this error is subsumed by `K0601` (interface
  declares >5 methods), which fires at the interface declaration site before
  constraint checking is reached; `K0807` exists as a safety net should `K0601`
  ever be relaxed.

Malformed generic syntax (e.g. mismatched angle brackets) is reported as a
syntax error under the existing code `K0003`.

## 8.7 Conformance cases this chapter introduces

These cases land in the following conformance PR (band `2xx` declarations, see
[`tests/conformance/README.md`](../../tests/conformance/README.md)):

| Case | Kind | Asserts |
|---|---|---|
| `223-generic-function` | accept | generic identity function parses and compiles |
| `224-generic-struct` | accept | generic struct with two type parameters |
| `225-constraint-satisfaction-structural` | accept | type satisfies interface bound without `impl` block |
| `226-generic-method-call` | accept | method call on generic type parameter through bound |
| `227-type-parameter-without-bound` | reject `K0801` | `[T]` without interface bound |
| `228-method-not-in-bound` | reject `K0802` | body calls method not in the bound |
| `229-type-argument-not-satisfying-bound` | reject `K0803` | concrete type missing a required method |
| `230-duplicate-type-parameter` | reject `K0804` | `[A: Foo, A: Foo]` with same name |
| `231-type-parameter-shadows-type` | reject `K0805` | `[Int: Foo]` where `Int` is a built-in |
| `232-too-many-type-parameters` | reject `K0806` | 257 type parameters |
| `233-constraint-interface-too-many-methods` | reject `K0601` | interface with six methods used as a bound (subsumed by K0601) |

## 8.8 Dependencies

- Decision: [`KDR-0022`](../kdr/0022-interface-constrained-generics.md).
- Frozen base: [`keel-core.md`](keel-core.md) §2 (types), §3 (declarations and
  function signatures), §4 (expressions).
- Interface system: [`07-interfaces.md`](07-interfaces.md) — constraints use
  interface syntax; runtime dispatch stays nominal.
- Code registry: `K0801`–`K0807` to be registered at implementation time in
  [`compiler/keelc-diag/src/registry.rs`](../../compiler/keelc-diag/src/registry.rs).

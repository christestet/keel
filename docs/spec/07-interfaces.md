# 07 — Interfaces

This chapter is **normative**. It adds **nominal interfaces** to the language,
the polymorphism mechanism recorded in [`KDR-0003`](../kdr/0003-no-inheritance.md).
It does not restate the frozen rules in [`keel-core.md`](keel-core.md); on any
conflict with `keel-core.md`, file an issue rather than reconciling silently
(the prime directive in the root [`AGENTS.md`](../../AGENTS.md)).

Interfaces in Keel are intentionally small and explicit:

- An interface declares a list of method signatures.
- A type implements an interface only via an explicit `impl Interface for Type`
  block.
- An interface may declare at most five methods (the limit is part of the
  decision, not a tuning parameter).
- There is no inheritance, no default method implementations, and no structural
  subtyping.

## 7.1 Interface declarations

An interface is declared at module level with the `interface` keyword:

```keel
interface Stringer {
    fn to_string(self) -> String
}
```

An interface body contains zero or more method signatures. Each signature uses
`fn` and must declare `self` as its first parameter, with no type annotation.
`self` is a contextual keyword inside interface and `impl` method signatures: it
names the receiver, and its type is the implementing type. Later parameters and
the return type follow normal Keel function-signature rules.

## 7.2 Implementing an interface

A type implements an interface with an `impl` block at module level:

```keel
struct Point {
    x: Int
    y: Int
}

impl Stringer for Point {
    fn to_string(self) -> String {
        "({self.x}, {self.y})"
    }
}
```

The `impl` block must supply exactly the methods declared by the interface, each
with a matching signature. The first parameter must be `self` with no type
annotation; inside the body `self` refers to the value whose method was called.

A type may implement multiple interfaces by writing multiple `impl` blocks. A
single interface may be implemented by multiple types.

## 7.3 Method calls

A method is called with receiver syntax:

```keel
let p = Point { x: 1, y: 2 }
let s = p.to_string()
```

The compiler resolves `p.to_string()` by finding an `impl` block for `Point`
that declares `to_string`. Method resolution is nominal: the call is valid only
because an explicit `impl` exists.

Methods may take additional parameters:

```keel
interface Scaler {
    fn scale(self, factor: Float) -> Point
}
```

## 7.4 Interfaces as types

An interface name may be used as a type. A value of a concrete type that
implements the interface may be passed or returned as the interface type:

```keel
fn show(it: Stringer) -> String {
    it.to_string()
}

fn make_stringer(x: Int, y: Int) -> Stringer {
    Point { x: x, y: y }
}
```

Calls through an interface value are dynamically dispatched.

## 7.5 Examples (normative, extracted by CI)

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

fn show(it: Stringer) -> String {
    it.to_string()
}

fn main() -> Unit {
    let p = Point { x: 3, y: 4 }
    print(p.to_string())
    print(show(p))
}
```

## 7.6 Error conditions

The following are errors with stable `K####` codes:

- **`K0601` — interface declares more than five methods.** A direct consequence
  of KDR-0003.
- **`K0602` — duplicate method name in interface.** The same identifier may not
  appear twice in one interface body.
- **`K0603` — missing method in impl.** An `impl` block for interface `I` omits
  a method declared by `I`.
- **`K0604` — method signature mismatch in impl.** A method in an `impl` block
  has a different name, parameter count, parameter type, or return type from the
  corresponding method in the interface. The first parameter must be `self` with
  no type annotation on both sides.
- **`K0605` — type does not implement interface.** A value of concrete type `T`
  is used where an interface `I` is required, but no `impl I for T` exists.
- **`K0606` — method not found in interface.** A method is called on an
  interface value, but the method is not declared by that interface.
- **`K0607` — extraneous method in impl.** An `impl` block declares a method that
  is not in the implemented interface.

Malformed interface or `impl` syntax (e.g. `self` with a type annotation, or a
method signature that is not a valid Keel function signature) is reported as a
syntax error under the existing code `K0003`.

## 7.7 Conformance cases this chapter introduces

These cases land in the following conformance PR (band `2xx` declarations, see
[`tests/conformance/README.md`](../../tests/conformance/README.md)):

| Case | Kind | Asserts |
|---|---|---|
| `212-interface-declaration` | accept | `interface Stringer { fn to_string(self) -> String }` parses |
| `213-impl-for-struct` | accept | `impl Stringer for Point` provides the required method |
| `214-interface-method-call` | accept | `p.to_string()` resolves through the explicit impl |
| `215-interface-pass-return` | accept | interfaces used as parameter and return types |
| `216-interface-too-many-methods` | reject `K0601` | interface with six methods is rejected |
| `217-interface-duplicate-method` | reject `K0602` | same identifier twice in one interface |
| `218-impl-missing-method` | reject `K0603` | impl omits an interface method |
| `219-impl-signature-mismatch` | reject `K0604` | parameter or return type differs from interface |
| `220-type-impl-mismatch` | reject `K0605` | concrete value assigned to interface it does not implement |
| `221-interface-method-not-found` | reject `K0606` | calling an undeclared method on an interface value |
| `222-impl-extra-method` | reject `K0607` | impl block declares a method not in the interface |

## 7.8 Dependencies

- Decision: [`KDR-0003`](../kdr/0003-no-inheritance.md).
- Frozen base: [`keel-core.md`](keel-core.md) §1 (keywords `interface` and
  `impl` are reserved for later milestones), §2 (types), §3 (declarations and
  function signatures).
- Code registry: `K0601`–`K0607` registered in
  [`compiler/keelc-diag/src/registry.rs`](../../compiler/keelc-diag/src/registry.rs)
  at spec-writing time ([`docs/spec/AGENTS.md`](AGENTS.md)).

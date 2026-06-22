# Syntax and specification index

Keel uses a hand-written recursive-descent parser. There is no separate EBNF
file because an unchecked second grammar would drift from the parser and
conformance suite. This index points each syntax area to its normative prose,
executable cases, and current implementation status.

## Source of truth order

1. Conformance cases define accepted/rejected observable behavior.
2. Normative spec chapters define the intended semantics those cases encode.
3. The parser/compiler implement that contract.

If a case and normative chapter disagree, stop and file an issue. Do not edit
one to match an assumption.

## Language index

| Area | Normative source | Executable/implementation anchor |
|---|---|---|
| UTF-8, identifiers, comments, literals, newlines | [Core §1](spec/keel-core.md#1-lexical-structure), [chapter 1](spec/01-lexical.md) | cases 001, 005–018; `keelc-lex` |
| Primitive and built-in types | [Core §2](spec/keel-core.md#2-types) | cases 101–122; `keelc-types`, `keelc-resolve` |
| Functions, structs, enums, bindings | [Core §3](spec/keel-core.md#3-declarations) | cases 201–234, 301–310; `keelc-parse` |
| Operators, precedence, blocks, `if`, `while`, `match` | [Core §4](spec/keel-core.md#4-expressions-and-control-flow), [chapter 4](spec/04-expressions.md) | cases 303–310, 401–411 |
| `Result`, `Option`, `?`, `catch`, union errors | [Core §5](spec/keel-core.md#5-errors) | cases 501–512 |
| Modules, manifests, path dependencies | [chapter 6](spec/06-modules-packages.md) | cases 601–602, 810–817; driver `manifest` module |
| Interfaces and `impl` | [chapter 7](spec/07-interfaces.md) | cases 212–222 |
| Constrained generics | [chapter 8](spec/08-generics.md) | cases 223–233 |
| `scope`, `spawn`, deadlines, cancellation | [chapter 9](spec/09-concurrency.md), [chapter 15 §§1–3](spec/15-stdlib-core.md#151-the-stdtime-module) | cases 710–723 |
| GC and `arena` | [chapter 10](spec/10-memory.md) | cases 830–833; partial Go-backend implementation |
| Package capabilities and audit | [chapter 11](spec/11-capabilities.md) | cases 820–828 |
| FFI / `extern` | chapter 12 not authored | `K0905`; planned M10 |
| Test blocks and assertions | [Core §7](spec/keel-core.md#7-entry-point-and-tests); chapter 13 not authored | cases 701–702 |
| Editions and previews | [chapter 14](spec/14-editions.md) | cases 840–842; preview case 843 blocked |
| Standard-library surface | [chapter 15](spec/15-stdlib-core.md) | cases 716–806 |

## Toolchain specification index

| Area | Normative source | Current status |
|---|---|---|
| LSP protocol | [chapter 16](spec/16-lsp.md) | specified, M8 implementation absent |
| Schema generation | [chapter 17](spec/17-codegen.md) | proto3 data subset implemented |
| Hermetic/reproducible builds | [chapter 18](spec/18-hermetic-builds.md) | fixed-input reproducibility implemented; SQL network gap remains |

## Parsed top-level forms

The M7 parser recognizes:

```text
module <name>
use <path>
struct <Name> { ... }
enum <Name> { ... }
interface <Name> { ... }
impl <Interface> for <Type> { ... }
fn <name>(...) -> <Type> { ... }
test "<name>" { ... }
```

Each source file is parsed as one module. The driver currently compiles one
source module at a time even when package manifests validate dependency/module
paths.

## Parsed statements and expressions

Conformance-backed statement/expression forms include:

- `let`, `mut`, assignment, `return`, `while`, `break`, and `continue`;
- blocks, calls, field access, struct literals, enum constructors, and closures
  used as HTTP handlers;
- `if` and exhaustive `match` expressions;
- prefix/binary operators from chapter 4;
- postfix `?`, `catch`, and `??` option defaulting;
- `scope`, `spawn`, and `arena` blocks;
- `assert` inside tests.

`for` is reserved lexically and appears in `impl Interface for Type`, but the
current AST/parser has no `for` loop statement despite Core listing `for`/`in`
among control-flow keywords. Treat loops as `while` until a normative case and
implementation resolve that gap.

## Why there is no copied grammar

The parser recovers from malformed input and milestone-gates syntax; a static
grammar alone would not express diagnostics, recovery, or feature availability.
A future formal grammar should be generated from or mechanically checked against
the parser and literate spec examples. Until that harness exists, this index and
conformance cases are safer than a plausible-looking but stale EBNF appendix.

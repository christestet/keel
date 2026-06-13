# Conformance test gap analysis

## Tested vs. untested features in Keel Core (keel-core.md)

### ✅ Adequately covered

| Area | Tests |
|------|-------|
| String interpolation | 010, 012–017 |
| Char literals | 104 |
| Bool literals/ops | 106, 115–117 |
| Int arithmetic | 107–114, 118, 121–122 |
| No implicit conversions | 101–102, 109, 120 |
| No semicolons | 008 |
| No null / no nil | 004, 103 |
| Identifier casing | 006–007 |
| Comments | 005 |
| `let` / `mut` | 202–204 |
| Struct fields + defaults | 002, 206–207, 210–211 |
| Fn signatures | 201, 205, 208 |
| Struct / enum + match | 301–308, 310 |
| `if` / `else` | 401–402, 408–409 |
| Block as expression | 403 |
| `while` / `break` / `continue` | 404–406, 410 |
| Early `return` | 407 |
| `?` operator | 501–503, 509 |
| `catch` / union errors | 504–508, 510 |
| Modules / `use` | 601–602 |
| Test blocks | 701 |
| Not-in-Core rejections | 901–909 |

### ❌ Missing or under-covered

| Feature | Spec ref | Gap |
|---------|----------|-----|
| `List<T>` type | §2 | No tests for list literals, construction, access, or methods |
| `Map<K, V>` type | §2 | No tests for map literals, construction, access, or methods |
| Block comments rejected | §1 | No reject test for `/* */` (should produce K0003 or a new K0103) |
| Reserved keywords as identifiers | §1 | No reject test for e.g. `let let = 1` (parser-level, may be covered by K0003 implicitly) |
| Integer overflow panic | §2 (K0203) | No runtime accept-test for overflow panic (e.g. `Int.max + 1`) |
| `break` outside loop | §4 | No reject test (should be a semantic error) |
| `continue` outside loop | §4 | No reject test |
| `for-in` loop not in Core | §1 keyword, not in §4 | No reject test — needs new K0909 |
| `impl` keyword not in Core | §1 reserved | No reject test — needs new K0910 |
| `panic` runtime behavior | §5 | No accept-test that panic exits with non-zero (M3+ runtime) |

### Priority for M3/M4

**High** (blocks backend or is a basic type): `List<T>`, `Map<K, V>` — these are in Core §2 and the compiler must handle them.

**Medium** (clear spec requirement): `for-in` reject (K0909), `impl` reject (K0910), `break`/`continue` outside loop.

**Low** (runtime behaviour or corner cases): overflow panic, block comments, reserved keywords as identifiers.

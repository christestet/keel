# M6 — `std.log` implementation

Non-normative implementation note. The contract is in
[`docs/spec/15-stdlib-core.md §15.25–15.27`](spec/15-stdlib-core.md).

## Status

**Done — all 3 conformance cases pass at M6.**

## Implementation surface

```keel
fn log.info(message: String) -> Unit
fn log.warn(message: String) -> Unit
fn log.error(message: String) -> Unit
```

Each writes to stdout with a `[level]` prefix. No error types, no structured
data, no filtering — YAGNI for M6.

## Touch points (compiler-known module, same pattern as `http`)

| Crate | What changed |
|---|---|
| `keelc-types/src/infer.rs` | `TypeContext::infer_call`: match `log.info\|warn\|error` → `Unit`. `infer_method_call`: same. |
| `keelc-resolve/src/lib.rs` | `infer_call`/`infer_method_call`: match `log` + `check_call_args(&[String], ...)`. Unknown methods emit `K0606`. |
| `keelc-backend-go/src/lib.rs` | `module_uses_log()`, `emit_log_call()`, `emit_log_runtime()`— three Go funcs calling `fmt.Println("[info]", msg)` etc. `uses_log` struct field. |

## Tests

| Case | What it checks |
|---|---|
| `746-log-info-output` | `log.info("hello")` → `[info] hello` |
| `747-log-warn-output` | `log.warn("careful")` → `[warn] careful` |
| `748-log-error-output` | `log.error("fail")` → `[error] fail` |

## Validation

```
KEEL_MILESTONE=M6 cargo run -p conformance-runner -- --keelc target/debug/keelc
# → 152 passed, 0 failed, 2 skipped (includes 746-748)
```

## Next

Structured key-value pairs or a context argument require a language feature
(named args or Map literals) that doesn't exist yet — deferred past M6.

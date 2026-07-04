# job-pipeline — structured concurrency + stdlib showcase

A second, standalone example for the 0.1.0 developer preview, alongside
[`hello.keel`](../hello.keel) (M3) and [`users-service`](../users-service/README.md)
(M6/M7, CRUD over SQL). This one demonstrates what "batteries included, no
`async`/`await`" looks like end to end without a database or network:

- an `interface` (`Job`) and a constrained generic function (`execute<T: Job>`)
- structured concurrency: a `scope(deadline: ...)` fans out three `spawn`ed
  jobs and joins them at the closing brace ([spec chapter 9](../../docs/spec/09-concurrency.md))
- `std.time` for the deadline, `std.json` to report a typed result, `std.log`
  for progress — the M6 stdlib slice
- ordinary `Result`/`catch` error handling, including the built-in `Cancelled`
  error if the deadline is ever exceeded

It targets **M6** language features only (M5 concurrency + M6 stdlib); it does
not use any M7 differentiator (manifests, capabilities, `arena`, `keel gen`,
editions).

Build and run it with the same toolchain used for the other examples:

```sh
./target/release/keel run examples/job-pipeline/main.keel --milestone M7
./target/release/keel fmt examples/job-pipeline/main.keel --milestone M7
./target/release/keel build examples/job-pipeline/main.keel --milestone M7
```

`keel run` prints:

```
[info] pipeline starting
[info] pipeline finished
{"alpha":9,"beta":25,"gamma":49,"total":83}
```

Like `users-service`, this is an example, not a conformance case: behavior
guarantees live in [`tests/conformance/`](../../tests/conformance/), not here.

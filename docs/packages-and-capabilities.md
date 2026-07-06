# Packages and capabilities

Packages are Keel's build and trust boundary. The normative definitions are
[spec chapter 6](spec/06-modules-packages.md) and
[spec chapter 11](spec/11-capabilities.md); conformance cases 810–828 lock the
current M7 slice.

## Single-file programs

A `.keel` file with no adjacent `keel.toml` is an implicit package. Its name is
the file stem, it has no dependencies, and it has no declared capabilities.

```sh
keel run hello.keel
keel audit hello.keel
```

An implicit package is useful for examples and small tools. Current M7 behavior
skips capability enforcement when no manifest exists, and `keel audit` reports
an empty set even if that file imports a capability-bearing standard module.
Therefore implicit packages are not an auditable security boundary; add a
manifest before relying on capability guarantees.

## Package layout

An explicit package is a directory rooted at `keel.toml`:

```text
service/
  keel.toml
  main.keel
  auth.keel
  helper/
    keel.toml
    email.keel
```

The current manifest schema is closed. Unknown sections and keys are `K1104`.

```toml
[package]
name = "service"
version = "0.1.0"
edition = "1"
capabilities = ["net", "env"]

[dependencies]
helper = { path = "helper" }
```

### Package fields

| Field | Required | Current meaning |
|---|---|---|
| `name` | yes | package identity; must be `snake_case` |
| `version` | yes | three-part `MAJOR.MINOR.PATCH` identity |
| `edition` | no | language edition; omitted means current edition (`1`) |
| `capabilities` | no | closed set of authority the package may exercise |

Only local path dependencies are implemented. The dependency alias is the name
used by `use`; each path must contain a readable `keel.toml`.

```keel
use helper.email
```

Dependency graphs must be acyclic, and two different directories cannot claim
the same package name. Diagnostics `K1106`–`K1108` cover unresolved paths,
cycles, and collisions.

## Current module ceiling

The driver scans every package source file for `use` paths, validates
dependency declarations, and computes capabilities. Cross-package **function
and type** references now link (spec §6.4,
[KDR-0044](kdr/0044-cross-package-symbol-linking.md)): a dependency module's
functions, structs, and enums are merged into the build; `module.fn(...)` and
`module.Type` annotations resolve to them, proven by `818-cross-package-call`
and `819-cross-package-type`. Still outside the ceiling: calls written inside
string interpolation (`"{dep.f()}"`), cross-package enum *variant-name*
collisions (variants are not mangled), and constructing a dependency struct
directly from the root (`dep.Point{...}` does not parse). Keep public package
APIs provisional until those land too.

## Capability declarations

Capabilities declare categories of authority, not individual resources:

| Capability | Authority |
|---|---|
| `net` | sockets and network services |
| `fs` | filesystem access |
| `exec` | child processes |
| `env` | environment variables |
| `ffi` | crossing an external ABI boundary |
| `unsafe-memory` | operations outside Keel's memory-safety guarantee |

The set is closed. An unknown capability is `K1111`.

Compiler-known standard-library modules require:

| Module | Required declaration |
|---|---|
| `std.http` | `net` |
| `std.sql` | `net`, `fs` |
| `std.config` | `env` |
| `std.time`, `std.json`, `std.log` | none |

Importing a capability-bearing module without its declaration is `K1110`:

```toml
[package]
name = "api"
version = "0.1.0"
capabilities = ["net"]
```

```keel
use std.http
```

Capability enforcement is transitive. If a dependency declares `net`, the
dependent package must also declare `net`, even when its own source never imports
`std.http`; otherwise the compiler emits `K1112`.

`ffi` and `unsafe-memory` can be declared and audited, but no implemented
language surface currently exercises them. `extern` remains `K0905` through M7.

## Audit a workspace

Pass an entry source file so `keel audit` can locate its manifest:

```sh
keel audit service/main.keel
```

Output is deterministic: capabilities use the fixed order above and contributing
packages are sorted by package name.

```text
users_service 0.1.0
  net: self, http_client 0.1.0
  (fs, exec, env, ffi, unsafe-memory: not present)
```

The report names effective authority and its contributors. A warning is emitted
when a package declares authority that neither its source nor its dependencies
exercise.

## Review checklist

When adding a dependency:

1. use the narrowest local path that contains its manifest;
2. inspect the dependency's declared capabilities;
3. add only the transitive capabilities required by the compiler;
4. run `keel audit` and review newly named contributors;
5. run the normal build/test gate.

Capabilities do not make dependency behavior trustworthy; they bound and expose
what the dependency can reach. Source review and version control still matter.

## Not implemented

- registry or Git dependencies;
- lockfiles and dependency resolution;
- package publishing;
- root-side construction of a dependency struct (`dep.Point{...}` does not parse) and cross-package enum variant-name collisions (functions, structs, and enums otherwise link — spec §6.4);
- function-level capability restrictions;
- FFI capability exercise and SBOM output.

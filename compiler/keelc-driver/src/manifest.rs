//! Package manifests (`keel.toml`) and capability enforcement.
//!
//! Implements spec chapters 06 (modules/packages) and 11 (capabilities) for the
//! M7 package slice: a tiny TOML-subset parser, the closed manifest schema, the
//! path-dependency graph, and static capability rollup. Every malformed input is
//! a `K11xx` diagnostic, never a panic (root AGENTS.md hard rule 6).
//!
//! Scope is deliberately small (ponytail): the only dependency form is a path
//! dependency, manifests are single-line TOML, and the only cross-package fact
//! we need from source is each file's `use` paths.

use keelc_parse::parse_with_milestone;
use keelc_span::SourceId;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// The six capabilities (spec §11.1), in their fixed reporting order.
const CAPABILITIES: [&str; 6] = ["net", "fs", "exec", "env", "ffi", "unsafe-memory"];

/// Editions the toolchain recognizes (spec ch14). Edition 1 is the only one.
const KNOWN_EDITIONS: [&str; 1] = ["1"];

/// Capabilities a `std` module obligates (spec §11.2). `None` => no capability.
fn std_module_caps(module: &str) -> &'static [&'static str] {
    match module {
        "http" => &["net"],
        "sql" => &["net", "fs"],
        "config" => &["env"],
        _ => &[], // time, json, log, and anything else: pure
    }
}

/// A diagnostic about a manifest or the package graph: stable code + message.
/// Spans into `keel.toml` are not tracked — these errors are about manifest
/// data, and the conformance contract matches on the code alone.
type ManifestDiag = (&'static str, String);

/// Run the M7 package + capability checks for the workspace rooted at the
/// directory of `entry`. Returns an empty vec for an implicit single-file
/// package (no adjacent `keel.toml`) — preserving all M0–M6 behavior.
#[must_use]
pub fn check_workspace(entry: &Path, milestone: u32) -> Vec<ManifestDiag> {
    let dir = entry.parent().filter(|p| !p.as_os_str().is_empty());
    let dir = dir.unwrap_or_else(|| Path::new("."));
    if !dir.join("keel.toml").is_file() {
        return Vec::new(); // implicit single-file package (spec §6.1)
    }

    let mut graph = Graph::new(milestone);
    graph.load(dir, &mut Vec::new());

    // Structural errors (manifest schema, graph shape) take precedence: a
    // broken graph makes capability rollup meaningless. Return them alone.
    if !graph.errors.is_empty() {
        graph.errors.sort();
        graph.errors.dedup();
        return graph.errors;
    }

    let mut errors = graph.capability_checks();
    errors.sort();
    errors.dedup();
    errors
}

/// A loaded package: its identity, declared capabilities, and path dependencies.
struct Package {
    dir: PathBuf,
    name: String,
    capabilities: BTreeSet<String>,
    /// alias -> canonical dependency directory.
    deps: BTreeMap<String, PathBuf>,
}

struct Graph {
    milestone: u32,
    packages: BTreeMap<PathBuf, Package>,
    names: BTreeMap<String, PathBuf>,
    done: BTreeSet<PathBuf>,
    errors: Vec<ManifestDiag>,
}

impl Graph {
    fn new(milestone: u32) -> Self {
        Self {
            milestone,
            packages: BTreeMap::new(),
            names: BTreeMap::new(),
            done: BTreeSet::new(),
            errors: Vec::new(),
        }
    }

    fn err(&mut self, code: &'static str, msg: impl Into<String>) {
        self.errors.push((code, msg.into()));
    }

    /// DFS over path dependencies. `stack` holds the canonical dirs currently
    /// being loaded, so a re-entry is a cycle (K1107).
    fn load(&mut self, dir: &Path, stack: &mut Vec<PathBuf>) {
        let canon = match dir.canonicalize() {
            Ok(c) => c,
            Err(_) => {
                self.err(
                    "K1106",
                    format!("dependency path `{}` is not readable", dir.display()),
                );
                return;
            }
        };
        if stack.contains(&canon) {
            self.err(
                "K1107",
                format!(
                    "dependency cycle through package at `{}`; break the cycle",
                    canon.display()
                ),
            );
            return;
        }
        if self.done.contains(&canon) {
            return; // diamond: already loaded, fine
        }

        let manifest_path = canon.join("keel.toml");
        let text = match std::fs::read_to_string(&manifest_path) {
            Ok(t) => t,
            Err(_) => {
                self.err(
                    "K1106",
                    format!(
                        "no readable `keel.toml` at dependency path `{}`",
                        canon.display()
                    ),
                );
                return;
            }
        };

        let raw = match parse_toml(&text) {
            Ok(raw) => raw,
            Err(msg) => {
                self.err(
                    "K1102",
                    format!("malformed manifest `{}`: {msg}", manifest_path.display()),
                );
                return;
            }
        };
        let pkg = match self.validate(&canon, raw) {
            Some(pkg) => pkg,
            None => return, // validation pushed the diagnostic(s)
        };

        // Name collision: two distinct directories claiming one name (K1108).
        if let Some(prev) = self.names.get(&pkg.name) {
            if prev != &canon {
                self.err(
                    "K1108",
                    format!(
                        "package name `{}` is declared by two different packages",
                        pkg.name
                    ),
                );
            }
        } else {
            self.names.insert(pkg.name.clone(), canon.clone());
        }

        stack.push(canon.clone());
        for depdir in pkg.deps.values() {
            if depdir.join("keel.toml").is_file() {
                self.load(depdir, stack);
            } else {
                self.err(
                    "K1106",
                    format!("dependency path `{}` has no `keel.toml`", depdir.display()),
                );
            }
        }
        stack.pop();

        self.done.insert(canon.clone());
        self.packages.insert(canon, pkg);
    }

    /// Validate a parsed manifest against the closed schema (spec §6.2),
    /// resolving dependency paths. Pushes K1102/K1103/K1104/K1111 as needed.
    fn validate(&mut self, dir: &Path, raw: RawManifest) -> Option<Package> {
        let mut ok = true;

        // Unknown top-level sections / keys outside the closed schema (K1104).
        for key in &raw.unknown_keys {
            self.err(
                "K1104",
                format!("unknown manifest key `{key}` (schema is closed)"),
            );
            ok = false;
        }

        let name = match raw.package.get("name") {
            Some(Value::String(s)) if is_snake_case(s) => s.clone(),
            Some(Value::String(s)) => {
                self.err(
                    "K1103",
                    format!("package name `{s}` is not a snake_case identifier"),
                );
                ok = false;
                String::new()
            }
            Some(_) => {
                self.err("K1103", "`[package].name` must be a string".to_string());
                ok = false;
                String::new()
            }
            None => {
                self.err("K1103", "`[package].name` is required".to_string());
                ok = false;
                String::new()
            }
        };

        match raw.package.get("version") {
            Some(Value::String(s)) if is_semver(s) => {}
            Some(Value::String(s)) => {
                self.err("K1103", format!("version `{s}` must be MAJOR.MINOR.PATCH"));
                ok = false;
            }
            Some(_) => {
                self.err("K1103", "`[package].version` must be a string".to_string());
                ok = false;
            }
            None => {
                self.err("K1103", "`[package].version` is required".to_string());
                ok = false;
            }
        }

        // Edition (spec ch14): omitted => current edition; a declared value the
        // toolchain does not recognize is K1401.
        match raw.package.get("edition") {
            None => {}
            Some(Value::String(s)) if KNOWN_EDITIONS.contains(&s.as_str()) => {}
            Some(Value::String(s)) => {
                self.err(
                    "K1401",
                    format!("edition `{s}` is not recognized by this toolchain"),
                );
                ok = false;
            }
            Some(_) => {
                self.err("K1102", "`[package].edition` must be a string".to_string());
                ok = false;
            }
        }

        // Capabilities: each entry must be one of the six (K1111).
        let mut capabilities = BTreeSet::new();
        if let Some(value) = raw.package.get("capabilities") {
            match value {
                Value::Array(items) => {
                    for cap in items {
                        if CAPABILITIES.contains(&cap.as_str()) {
                            capabilities.insert(cap.clone());
                        } else {
                            self.err(
                                "K1111",
                                format!("`{cap}` is not one of the six capabilities"),
                            );
                            ok = false;
                        }
                    }
                }
                _ => {
                    self.err(
                        "K1102",
                        "`capabilities` must be an array of strings".to_string(),
                    );
                    ok = false;
                }
            }
        }

        // Path dependencies: resolve each alias to a directory (no I/O yet).
        let mut deps = BTreeMap::new();
        for (alias, dep) in &raw.dependencies {
            match dep.get("path") {
                Some(Value::String(rel)) => {
                    deps.insert(alias.clone(), dir.join(rel));
                }
                _ => {
                    self.err(
                        "K1102",
                        format!("dependency `{alias}` must be `{{ path = \"...\" }}`"),
                    );
                    ok = false;
                }
            }
        }

        if ok {
            Some(Package {
                dir: dir.to_path_buf(),
                name,
                capabilities,
                deps,
            })
        } else {
            None
        }
    }

    /// Phase 2 (graph is sound): `use`-path resolution (K1105) and capability
    /// enforcement, direct (K1110) and transitive (K1112).
    fn capability_checks(&self) -> Vec<ManifestDiag> {
        let mut errors = Vec::new();
        for pkg in self.packages.values() {
            let aliases: BTreeSet<&str> = pkg.deps.keys().map(String::as_str).collect();
            for path in package_use_paths(pkg, self.milestone) {
                let first = path.first().map(String::as_str).unwrap_or_default();
                if first == "std" {
                    let module = path.get(1).map(String::as_str).unwrap_or_default();
                    for cap in std_module_caps(module) {
                        if !pkg.capabilities.contains(*cap) {
                            errors.push((
                                "K1110",
                                format!(
                                    "package `{}` uses `std.{module}` which requires capability `{cap}`; declare it",
                                    pkg.name
                                ),
                            ));
                        }
                    }
                } else if first != pkg.name && !aliases.contains(first) {
                    errors.push((
                        "K1105",
                        format!(
                            "`use {first}...` in package `{}` names neither std, the package, nor a dependency",
                            pkg.name
                        ),
                    ));
                }
            }

            // Transitive: a package must declare every capability its direct
            // dependencies declare (spec §11.3).
            for depdir in pkg.deps.values() {
                if let Some(dep) = self.packages.get(depdir) {
                    for cap in &dep.capabilities {
                        if !pkg.capabilities.contains(cap) {
                            errors.push((
                                "K1112",
                                format!(
                                    "package `{}` depends on `{}` which requires `{cap}`; declare it too",
                                    pkg.name, dep.name
                                ),
                            ));
                        }
                    }
                }
            }
        }
        errors
    }
}

/// Collect every `use` path of every `.keel` file in the package's directory
/// tree, stopping at subdirectories that root their own package (spec §6.1).
fn package_use_paths(pkg: &Package, milestone: u32) -> Vec<Vec<String>> {
    let mut files = Vec::new();
    collect_keel_files(&pkg.dir, true, &mut files);
    files.sort(); // determinism
    let mut paths = Vec::new();
    for file in files {
        let Ok(text) = std::fs::read_to_string(&file) else {
            continue;
        };
        let module = parse_with_milestone(SourceId::new(0), &text, milestone).module;
        for item in &module.items {
            if let keelc_ast::Item::Use(decl) = item {
                paths.push(decl.path.iter().map(|s| s.value.clone()).collect());
            }
        }
    }
    paths
}

fn collect_keel_files(dir: &Path, is_root: bool, out: &mut Vec<PathBuf>) {
    // A nested keel.toml roots a different package — don't descend into it.
    if !is_root && dir.join("keel.toml").is_file() {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_keel_files(&path, false, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("keel") {
            out.push(path);
        }
    }
}

// ---------- the TOML subset ----------

#[derive(Debug, PartialEq)]
enum Value {
    String(String),
    Array(Vec<String>),
    Table(BTreeMap<String, Value>),
}

impl Value {
    fn get(&self, key: &str) -> Option<&Value> {
        match self {
            Value::Table(t) => t.get(key),
            _ => None,
        }
    }
}

/// What we extract from a manifest before schema validation.
#[derive(Default, Debug)]
struct RawManifest {
    package: BTreeMap<String, Value>,
    dependencies: BTreeMap<String, Value>,
    /// keys/sections outside the closed schema (drives K1104).
    unknown_keys: Vec<String>,
}

/// Parse the single-line TOML subset that `keel.toml` uses. Returns the raw
/// key/value structure, or a message describing why it is not valid TOML
/// (surfaced as K1102). Never panics.
fn parse_toml(text: &str) -> Result<RawManifest, String> {
    let mut raw = RawManifest::default();
    let mut section = String::new();
    for line in text.lines() {
        let line = strip_comment(line).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            section = name.trim().to_string();
            if section != "package" && section != "dependencies" {
                raw.unknown_keys.push(format!("[{section}]"));
            }
            continue;
        }
        let (key, rest) = line
            .split_once('=')
            .ok_or_else(|| format!("expected `key = value`, got `{line}`"))?;
        let key = key.trim().to_string();
        let value = parse_value(rest.trim())?;
        match section.as_str() {
            "package" => {
                if matches!(
                    key.as_str(),
                    "name" | "version" | "edition" | "capabilities"
                ) {
                    raw.package.insert(key, value);
                } else {
                    raw.unknown_keys.push(key);
                }
            }
            "dependencies" => {
                raw.dependencies.insert(key, value);
            }
            "" => return Err(format!("key `{key}` outside any [section]")),
            _ => {} // keys under an unknown section: the section itself is already K1104
        }
    }
    Ok(raw)
}

/// Remove a `#` comment, ignoring `#` inside a double-quoted string.
fn strip_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut in_string = false;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => in_string = !in_string,
            b'\\' if in_string => i += 1, // skip escaped char
            b'#' if !in_string => return &line[..i],
            _ => {}
        }
        i += 1;
    }
    line
}

fn parse_value(s: &str) -> Result<Value, String> {
    if let Some(rest) = s.strip_prefix('"') {
        let end = rest
            .find('"')
            .ok_or_else(|| format!("unterminated string `{s}`"))?;
        return Ok(Value::String(rest[..end].to_string()));
    }
    if let Some(inner) = s.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
        let mut items = Vec::new();
        for part in inner.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            match parse_value(part)? {
                Value::String(v) => items.push(v),
                _ => return Err(format!("array elements must be strings, got `{part}`")),
            }
        }
        return Ok(Value::Array(items));
    }
    if let Some(inner) = s.strip_prefix('{').and_then(|s| s.strip_suffix('}')) {
        let mut table = BTreeMap::new();
        for part in inner.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }
            let (k, v) = part
                .split_once('=')
                .ok_or_else(|| format!("expected `key = value` in `{part}`"))?;
            table.insert(k.trim().to_string(), parse_value(v.trim())?);
        }
        return Ok(Value::Table(table));
    }
    Err(format!(
        "unsupported value `{s}` (only strings, arrays, and inline tables)"
    ))
}

fn is_snake_case(s: &str) -> bool {
    let mut bytes = s.bytes();
    matches!(bytes.next(), Some(b) if b == b'_' || b.is_ascii_lowercase())
        && bytes.all(|b| b == b'_' || b.is_ascii_lowercase() || b.is_ascii_digit())
}

fn is_semver(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    parts.len() == 3
        && parts
            .iter()
            .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_manifest() {
        let raw = parse_toml("[package]\nname = \"x\"\nversion = \"0.1.0\"\n").unwrap();
        assert_eq!(raw.package.get("name"), Some(&Value::String("x".into())));
        assert!(raw.unknown_keys.is_empty());
    }

    #[test]
    fn unterminated_string_is_error() {
        assert!(parse_toml("[package]\nname = \"oops\nversion = \"0.1.0\"\n").is_err());
    }

    #[test]
    fn strips_trailing_comment_not_inside_string() {
        let raw = parse_toml("[package]\nname = \"a#b\" # note\nversion = \"0.1.0\"\n").unwrap();
        assert_eq!(raw.package.get("name"), Some(&Value::String("a#b".into())));
    }

    #[test]
    fn unknown_key_recorded() {
        let raw = parse_toml("[package]\nauthors = [\"x\"]\n").unwrap();
        assert_eq!(raw.unknown_keys, vec!["authors".to_string()]);
    }

    #[test]
    fn capabilities_and_deps_parse() {
        let raw =
            parse_toml("[package]\ncapabilities = [\"net\", \"fs\"]\n[dependencies]\nh = { path = \"helper\" }\n")
                .unwrap();
        assert_eq!(
            raw.package.get("capabilities"),
            Some(&Value::Array(vec!["net".into(), "fs".into()]))
        );
        assert!(raw.dependencies.contains_key("h"));
    }

    #[test]
    fn semver_and_snake_case() {
        assert!(is_semver("0.1.0"));
        assert!(!is_semver("0.1"));
        assert!(is_snake_case("users_service"));
        assert!(!is_snake_case("Users"));
    }
}

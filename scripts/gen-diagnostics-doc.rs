// Regenerates the "Registered codes" table in docs/diagnostics.md from the
// single source of truth, compiler/keelc-diag/src/registry.rs, so the doc
// cannot silently drift from the append-only code registry (AGENTS.md hard
// rule 4). Compiled directly with rustc, like scripts/check-docs.rs, so it
// adds no project dependency.
//
// Usage:
//   scripts/gen-diagnostics-doc.rs           # regenerate the file in place
//   scripts/gen-diagnostics-doc.rs --check   # fail if the file is stale

use std::fs;
use std::path::PathBuf;
use std::process::Command;

const START: &str = "<!-- gen:diagnostics:start -->";
const END: &str = "<!-- gen:diagnostics:end -->";

fn main() {
    let check_only = std::env::args().nth(1).as_deref() == Some("--check");
    let root = repository_root();
    let registry_path = root.join("compiler/keelc-diag/src/registry.rs");
    let doc_path = root.join("docs/diagnostics.md");

    let registry = fs::read_to_string(&registry_path)
        .unwrap_or_else(|error| fail(&format!("cannot read {}: {error}", registry_path.display())));
    let entries = parse_registry(&registry);

    let doc = fs::read_to_string(&doc_path)
        .unwrap_or_else(|error| fail(&format!("cannot read {}: {error}", doc_path.display())));
    let updated = replace_table(&doc, &entries);

    if updated == doc {
        println!("diagnostics-doc: ok ({} codes)", entries.len());
        return;
    }

    if check_only {
        eprintln!(
            "diagnostics-doc: FAIL docs/diagnostics.md is stale relative to \
             compiler/keelc-diag/src/registry.rs; run \
             scripts/gen-diagnostics-doc.rs --write"
        );
        std::process::exit(1);
    }

    fs::write(&doc_path, updated)
        .unwrap_or_else(|error| fail(&format!("cannot write {}: {error}", doc_path.display())));
    println!("diagnostics-doc: wrote {} codes", entries.len());
}

fn parse_registry(source: &str) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    let mut in_block = false;

    for line in source.lines() {
        let line = line.trim();
        if !in_block {
            if line.starts_with("codes! {") {
                in_block = true;
            }
            continue;
        }
        if line == "}" {
            break;
        }
        let Some((name, rest)) = line.split_once("=>") else {
            continue;
        };
        let name = name.trim();
        let literal = rest.trim().trim_end_matches(',').trim();
        let Some(summary) = literal.strip_prefix('"').and_then(|s| s.strip_suffix('"')) else {
            fail(&format!("registry.rs: cannot parse summary literal `{literal}`"));
        };
        entries.push((name.to_owned(), summary.replace("\\\"", "\"")));
    }

    if entries.is_empty() {
        fail("registry.rs: found no codes! entries — registry format changed?");
    }
    entries
}

fn replace_table(doc: &str, entries: &[(String, String)]) -> String {
    let start = doc
        .find(START)
        .unwrap_or_else(|| fail("docs/diagnostics.md: missing gen:diagnostics:start marker"));
    let end = doc
        .find(END)
        .unwrap_or_else(|| fail("docs/diagnostics.md: missing gen:diagnostics:end marker"));

    let mut table = String::new();
    table.push_str(START);
    table.push('\n');
    table.push_str("| Code | Registry summary |\n");
    table.push_str("|---|---|\n");
    for (code, summary) in entries {
        table.push_str(&format!("| `{code}` | {summary} |\n"));
    }

    let mut updated = String::new();
    updated.push_str(&doc[..start]);
    updated.push_str(&table);
    updated.push_str(&doc[end..]);
    updated
}

fn repository_root() -> PathBuf {
    let output = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .unwrap_or_else(|error| fail(&format!("cannot run git: {error}")));
    if !output.status.success() {
        fail("not inside a Git repository");
    }
    PathBuf::from(String::from_utf8_lossy(&output.stdout).trim())
}

fn fail(message: &str) -> ! {
    eprintln!("diagnostics-doc: FAIL {message}");
    std::process::exit(1)
}

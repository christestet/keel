//! Conformance runner — executes `tests/conformance/` against a keelc binary,
//! or, when no compiler exists yet (pre-M1) / `--check` is passed, lints the
//! suite's *structure* so the executable spec itself can't rot.
//!
//! Contract with keelc (the runner side of compiler/ARCHITECTURE.md):
//!   * M1 syntax mode (`--milestone M1`): `keelc check <main.keel>` exits 0 for
//!     parseable cases; syntax-stage reject-cases must emit the expected code.
//!   * M3+ run mode: `keelc run <main.keel>` exits 0; stdout must equal
//!     `expected.stdout` (trailing-newline normalized).
//!   * reject-case: keelc exits non-zero and stderr contains the diagnostic
//!     code from line 1 of `expected.error` (e.g. `K0301`). If line 2 is
//!     `line:N`, stderr must also contain `main.keel:N`. Message TEXT is
//!     never matched — codes are the stable API.
//!
//! Usage:
//!   conformance-runner [--check] [--suite <dir>] [--keelc <path>] [--milestone M2]
//!   env fallbacks: KEELC, KEEL_MILESTONE; default milestone: M1
//!
//! Exit codes: 0 = all green (or structure ok), 1 = failures, 2 = suite malformed.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

// ---------- case model ----------

#[derive(Debug)]
enum Expectation {
    Stdout(String),
    Error { code: String, line: Option<u32> },
}

#[derive(Debug)]
struct Case {
    name: String,
    dir: PathBuf,
    expectation: Expectation,
    /// Minimum milestone (e.g. 2 for "M2") at which this case must pass.
    milestone: Option<u32>,
}

#[derive(Debug)]
struct StructureError {
    case: String,
    problem: String,
}

// ---------- discovery & structural linting ----------

fn discover(suite: &Path) -> Result<Vec<Case>, Vec<StructureError>> {
    let mut cases = Vec::new();
    let mut errors = Vec::new();
    let mut seen_numbers: Vec<(String, String)> = Vec::new(); // (number, name)

    let entries = match fs::read_dir(suite) {
        Ok(e) => e,
        Err(e) => {
            return Err(vec![StructureError {
                case: suite.display().to_string(),
                problem: format!("cannot read suite directory: {e}"),
            }])
        }
    };

    let mut dirs: Vec<PathBuf> = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort(); // determinism: iron rule 7

    for dir in dirs {
        let name = dir.file_name().unwrap().to_string_lossy().to_string();
        let mut err = |problem: String| {
            errors.push(StructureError {
                case: name.clone(),
                problem,
            });
        };

        // NNN-kebab-name
        let valid_name = name.len() > 4
            && name.as_bytes()[..3].iter().all(u8::is_ascii_digit)
            && name.as_bytes()[3] == b'-'
            && name[4..]
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
        if !valid_name {
            err("directory name must match NNN-kebab-name (e.g. 042-match-guards)".into());
            continue;
        }
        let number = name[..3].to_string();
        if let Some((_, prev)) = seen_numbers.iter().find(|(n, _)| *n == number) {
            err(format!(
                "case number {number} already used by {prev} (numbers are permanent and unique)"
            ));
        }
        seen_numbers.push((number, name.clone()));

        if !dir.join("main.keel").is_file() {
            err("missing main.keel".into());
            continue;
        }

        let stdout_p = dir.join("expected.stdout");
        let error_p = dir.join("expected.error");
        let expectation = match (stdout_p.is_file(), error_p.is_file()) {
            (true, true) => {
                err("has BOTH expected.stdout and expected.error — exactly one is required".into());
                continue;
            }
            (false, false) => {
                err(
                    "has NEITHER expected.stdout nor expected.error — exactly one is required"
                        .into(),
                );
                continue;
            }
            (true, false) => Expectation::Stdout(normalize(&read(&stdout_p))),
            (false, true) => match parse_expected_error(&read(&error_p)) {
                Ok(exp) => exp,
                Err(p) => {
                    err(format!("expected.error: {p}"));
                    continue;
                }
            },
        };

        // optional case.toml — only `milestone = "MN"` is recognized; hand-parsed
        // to keep the runner dependency-free.
        let milestone = match parse_case_toml(&dir.join("case.toml")) {
            Ok(m) => m,
            Err(p) => {
                err(format!("case.toml: {p}"));
                continue;
            }
        };

        cases.push(Case {
            name,
            dir,
            expectation,
            milestone,
        });
    }

    if errors.is_empty() {
        Ok(cases)
    } else {
        Err(errors)
    }
}

fn read(p: &Path) -> String {
    fs::read_to_string(p).unwrap_or_default()
}

/// Trailing-newline normalization: exactly one trailing '\n'.
fn normalize(s: &str) -> String {
    let mut s = s.replace("\r\n", "\n");
    while s.ends_with('\n') {
        s.pop();
    }
    s.push('\n');
    s
}

fn parse_expected_error(text: &str) -> Result<Expectation, String> {
    let mut lines = text.lines().filter(|l| !l.trim().is_empty());
    let code = lines
        .next()
        .ok_or("empty file; first line must be a K#### code")?
        .trim()
        .to_string();
    let ok_code =
        code.len() == 5 && code.starts_with('K') && code[1..].bytes().all(|b| b.is_ascii_digit());
    if !ok_code {
        return Err(format!(
            "first line must be a diagnostic code like K0301, got `{code}`"
        ));
    }
    let line = match lines.next() {
        None => None,
        Some(l) => {
            let l = l.trim();
            let n = l
                .strip_prefix("line:")
                .ok_or_else(|| format!("second line must be `line:N`, got `{l}`"))?;
            Some(
                n.parse::<u32>()
                    .map_err(|_| format!("`line:{n}` is not a number"))?,
            )
        }
    };
    if lines.next().is_some() {
        return Err("at most two lines allowed (code, then optional line:N)".into());
    }
    Ok(Expectation::Error { code, line })
}

fn parse_case_toml(p: &Path) -> Result<Option<u32>, String> {
    if !p.is_file() {
        return Ok(None);
    }
    for raw in read(p).lines() {
        let l = raw.split('#').next().unwrap_or("").trim();
        if l.is_empty() {
            continue;
        }
        if let Some(v) = l.strip_prefix("milestone") {
            let v = v
                .trim_start()
                .strip_prefix('=')
                .ok_or("expected `milestone = \"MN\"`")?;
            let v = v.trim().trim_matches('"');
            return Ok(Some(parse_milestone(v)?));
        }
        return Err(format!(
            "unrecognized key in `{l}` (only `milestone` is allowed)"
        ));
    }
    Ok(None)
}

fn parse_milestone(s: &str) -> Result<u32, String> {
    s.strip_prefix('M')
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| format!("milestone must look like M3, got `{s}`"))
}

fn resolve_keelc_arg(raw: String) -> String {
    let path = Path::new(&raw);
    let looks_path_like = path.is_absolute() || raw.contains('/') || raw.contains('\\');
    if looks_path_like {
        path.canonicalize()
            .map(|path| path.display().to_string())
            .unwrap_or(raw)
    } else {
        raw
    }
}

// ---------- execution ----------

enum Outcome {
    Pass,
    Skip(String),
    Fail(String),
}

fn run_case(case: &Case, keelc: &str, current_milestone: Option<u32>) -> Outcome {
    if let (Some(need), Some(cur)) = (case.milestone, current_milestone) {
        if need > cur {
            return Outcome::Skip(format!("requires M{need}, running at M{cur}"));
        }
    }

    if current_milestone == Some(1) {
        return check_m1_syntax(case, keelc);
    }

    let out = match invoke_keelc(case, keelc, "run") {
        Ok(o) => o,
        Err(e) => return Outcome::Fail(format!("could not invoke `{keelc}`: {e}")),
    };
    let stdout = normalize(&String::from_utf8_lossy(&out.stdout));
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    match &case.expectation {
        Expectation::Stdout(want) => {
            if !out.status.success() {
                return Outcome::Fail(format!(
                    "expected successful run, keelc exited {:?}\n--- stderr ---\n{stderr}",
                    out.status.code()
                ));
            }
            if &stdout != want {
                return Outcome::Fail(diff("stdout", want, &stdout));
            }
            Outcome::Pass
        }
        Expectation::Error { code, line } => {
            if out.status.success() {
                return Outcome::Fail(format!(
                    "expected compile error {code}, but program ran successfully"
                ));
            }
            if !stderr.contains(code.as_str()) {
                return Outcome::Fail(format!(
                    "expected diagnostic {code} in stderr\n--- stderr ---\n{stderr}"
                ));
            }
            if let Some(n) = line {
                let needle = format!("main.keel:{n}");
                if !stderr.contains(&needle) {
                    return Outcome::Fail(format!(
                        "expected primary span at {needle}\n--- stderr ---\n{stderr}"
                    ));
                }
            }
            Outcome::Pass
        }
    }
}

fn check_m1_syntax(case: &Case, keelc: &str) -> Outcome {
    let out = match invoke_keelc(case, keelc, "check") {
        Ok(o) => o,
        Err(e) => return Outcome::Fail(format!("could not invoke `{keelc}`: {e}")),
    };
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    match &case.expectation {
        Expectation::Stdout(_) => {
            if out.status.success() {
                Outcome::Pass
            } else {
                Outcome::Fail(format!(
                    "expected successful M1 syntax check, keelc exited {:?}\n--- stderr ---\n{stderr}",
                    out.status.code()
                ))
            }
        }
        Expectation::Error { code, line } if is_m1_syntax_code(code) => {
            if out.status.success() {
                return Outcome::Fail(format!(
                    "expected M1 syntax diagnostic {code}, but check succeeded"
                ));
            }
            if !stderr.contains(code.as_str()) {
                return Outcome::Fail(format!(
                    "expected diagnostic {code} in stderr\n--- stderr ---\n{stderr}"
                ));
            }
            if let Some(n) = line {
                let needle = format!("main.keel:{n}");
                if !stderr.contains(&needle) {
                    return Outcome::Fail(format!(
                        "expected primary span at {needle}\n--- stderr ---\n{stderr}"
                    ));
                }
            }
            Outcome::Pass
        }
        Expectation::Error { code, .. } => {
            if out.status.success() {
                Outcome::Pass
            } else {
                Outcome::Fail(format!(
                    "expected M1 parse success for later-stage diagnostic {code}\n--- stderr ---\n{stderr}"
                ))
            }
        }
    }
}

fn invoke_keelc(case: &Case, keelc: &str, command: &str) -> std::io::Result<std::process::Output> {
    Command::new(keelc)
        .arg(command)
        .arg("main.keel")
        .current_dir(&case.dir)
        .output()
}

fn is_m1_syntax_code(code: &str) -> bool {
    matches!(
        code,
        "K0001"
            | "K0002"
            | "K0003"
            | "K0004"
            | "K0101"
            | "K0102"
            | "K0201"
            | "K0302"
            | "K0901"
            | "K0902"
            | "K0903"
            | "K0904"
            | "K0905"
            | "K0906"
            | "K0907"
            | "K0908"
    )
}

fn diff(what: &str, want: &str, got: &str) -> String {
    let mut s = format!("{what} mismatch\n");
    let (w, g): (Vec<_>, Vec<_>) = (want.lines().collect(), got.lines().collect());
    for i in 0..w.len().max(g.len()) {
        let (w_l, g_l) = (
            w.get(i).copied().unwrap_or(""),
            g.get(i).copied().unwrap_or(""),
        );
        if w_l == g_l {
            let _ = writeln!(s, "    {w_l}");
        } else {
            let _ = writeln!(s, "  - {w_l}");
            let _ = writeln!(s, "  + {g_l}");
        }
    }
    s
}

// ---------- CLI ----------

fn main() -> ExitCode {
    let mut suite = PathBuf::from("tests/conformance");
    let mut keelc = std::env::var("KEELC").ok();
    let mut milestone = std::env::var("KEEL_MILESTONE")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .or_else(|| Some("M1".to_string()));
    let mut check_only = false;

    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--check" => check_only = true,
            "--suite" => suite = PathBuf::from(args.next().expect("--suite needs a path")),
            "--keelc" => keelc = Some(args.next().expect("--keelc needs a path")),
            "--milestone" => milestone = Some(args.next().expect("--milestone needs M<N>")),
            other => {
                eprintln!("unknown argument: {other}");
                return ExitCode::from(2);
            }
        }
    }

    // 1. Structure first — a malformed suite is worse than a failing one.
    let cases = match discover(&suite) {
        Ok(c) => c,
        Err(errs) => {
            eprintln!("suite structure is invalid ({} problem(s)):\n", errs.len());
            for e in &errs {
                eprintln!("  ✗ {}: {}", e.case, e.problem);
            }
            return ExitCode::from(2);
        }
    };
    println!("suite ok: {} case(s), structure valid", cases.len());

    let Some(keelc) = keelc else {
        if check_only {
            return ExitCode::SUCCESS;
        }
        println!("no compiler given (set KEELC or pass --keelc); structure check only");
        return ExitCode::SUCCESS;
    };
    if check_only {
        return ExitCode::SUCCESS;
    }

    let keelc = resolve_keelc_arg(keelc);
    let current_milestone = milestone
        .as_deref()
        .map(|m| parse_milestone(m).expect("bad --milestone"));

    // 2. Execute.
    let (mut pass, mut fail, mut skip) = (0u32, 0u32, 0u32);
    for case in &cases {
        match run_case(case, &keelc, current_milestone) {
            Outcome::Pass => {
                pass += 1;
                println!("  ✓ {}", case.name);
            }
            Outcome::Skip(why) => {
                skip += 1;
                println!("  - {} (skipped: {why})", case.name);
            }
            Outcome::Fail(why) => {
                fail += 1;
                println!("  ✗ {}", case.name);
                for l in why.lines() {
                    println!("      {l}");
                }
            }
        }
    }

    println!("\n  {pass} passed, {fail} failed, {skip} skipped");
    if fail == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

//! Conformance runner — executes `tests/conformance/` against a keelc binary,
//! or, when no compiler exists yet (pre-M1) / `--check` is passed, lints the
//! suite's *structure* so the executable spec itself can't rot.
//!
//! Contract with keelc (the runner side of compiler/ARCHITECTURE.md):
//!   * M1 syntax mode (`--milestone M1`): `keelc check <main.keel>` exits 0 for
//!     parseable cases; syntax-stage reject-cases must emit the expected code.
//!   * M2 semantic mode (`--milestone M2`): `keelc check <main.keel>` exits 0
//!     for accepted cases; reject-cases must emit the expected code.
//!   * M3+ run mode: `keelc run <main.keel>` exits 0; stdout must equal
//!     `expected.stdout` (trailing-newline normalized).
//!   * M4+ test mode: `case.toml` may set `mode = "test"`; the runner invokes
//!     `keelc test <main.keel>` and matches stdout against `expected.stdout`.
//!   * M4+ build mode: `case.toml` may set `mode = "build"`; the runner invokes
//!     `keelc build <main.keel>`, runs the produced binary, and matches stdout.
//!   * M7+ audit mode: `case.toml` may set `mode = "audit"`; the runner invokes
//!     `keelc audit <main.keel>` and matches its stdout (the capability report).
//!   * reject-case: keelc exits non-zero and stderr contains the diagnostic
//!     code from line 1 of `expected.error` (e.g. `K0301`). If line 2 is
//!     `line:N`, stderr must also contain `main.keel:N`. Message TEXT is
//!     never matched — codes are the stable API.
//!   * warning-case: optional `expected.warning` alongside `expected.stdout`;
//!     program must compile and its stderr must contain the warning code.
//!
//! Usage:
//!   conformance-runner [--check] [--suite <dir>] [--keelc <path>] [--milestone M2]
//!   env fallbacks: KEELC, KEEL_MILESTONE; default milestone: M1
//!
//! Exit codes: 0 = all green (or structure ok), 1 = failures, 2 = suite malformed.

mod json;

use json::Value;
use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

// ---------- case model ----------

#[derive(Debug, PartialEq)]
enum Expectation {
    Stdout(String),
    Error {
        code: String,
        line: Option<u32>,
    },
    /// Build-only: compilation must succeed; no binary execution or stdout check.
    BuildOnly,
}

#[derive(Debug)]
struct WarningCheck {
    code: String,
    line: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RunMode {
    Run,
    Test,
    Build,
    /// `keelc audit main.keel`: the report goes to stdout, matched like Run.
    Audit,
    /// `keelc gen schema.proto`: generated Keel goes to stdout, matched like
    /// Run, and is additionally re-formatted to assert `keel fmt`-idempotence
    /// (spec ch17). The case carries `schema.proto`, not `main.keel`.
    Gen,
    /// `keelc build` run twice: the two binaries must be byte-identical, then
    /// one is run and its stdout matched (spec ch18, hermetic builds).
    Repro,
    /// `keelc build --image` run twice: the two OCI Image Layouts must be
    /// byte-identical, then one is validated structurally (spec ch19,
    /// KDR-0107).
    Image,
}

#[derive(Debug)]
struct Case {
    name: String,
    dir: PathBuf,
    expectation: Expectation,
    /// Minimum milestone (e.g. 2 for "M2") at which this case must pass.
    milestone: Option<u32>,
    /// Maximum milestone (e.g. 4 for "M4") at which this case must pass.
    /// Cases with an `until` are skipped once the milestone exceeds it.
    until: Option<u32>,
    /// Optional expected warning (for accept-cases that also emit a warning).
    expected_warning: Option<WarningCheck>,
    /// How to execute an accept-case: `keelc run` (default) or `keelc test`.
    mode: RunMode,
    /// Image mode only: target arch passed as `--arch <v>` and asserted against
    /// the config's `architecture` field (spec §19.2/§19.8). `None` = default.
    arch: Option<String>,
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

        // optional case.toml — `milestone = "MN"`, `until = "MN"`,
        // `mode = "run|test|build|audit|gen|repro|image"`, and (image only)
        // `arch = "amd64|arm64"` are recognized; hand-parsed to stay
        // dependency-free.
        let (milestone, until, mode, arch) = match parse_case_toml(&dir.join("case.toml")) {
            Ok(parsed) => parsed,
            Err(p) => {
                err(format!("case.toml: {p}"));
                continue;
            }
        };

        // Most cases drive `main.keel`; `gen` cases drive a schema file instead.
        let input = if mode == RunMode::Gen {
            "schema.proto"
        } else {
            "main.keel"
        };
        if !dir.join(input).is_file() {
            err(format!("missing {input}"));
            continue;
        }

        // Image mode has no way to "run" the produced OCI artifact directly,
        // so like Build it may omit expected output and just assert success.
        let is_build_mode_no_stdout = mode == RunMode::Build || mode == RunMode::Image;

        let stdout_p = dir.join("expected.stdout");
        let error_p = dir.join("expected.error");
        let warning_p = dir.join("expected.warning");

        if warning_p.is_file() && error_p.is_file() {
            err("cannot have both expected.warning and expected.error".into());
            continue;
        }

        let expectation = match (stdout_p.is_file(), error_p.is_file()) {
            (true, true) => {
                err("has BOTH expected.stdout and expected.error — exactly one is required".into());
                continue;
            }
            (false, false) => {
                if is_build_mode_no_stdout {
                    // Build-mode tests can omit expected output —
                    // they only verify that compilation succeeds.
                    Expectation::BuildOnly
                } else {
                    err(
                        "has NEITHER expected.stdout nor expected.error — exactly one is required"
                            .into(),
                    );
                    continue;
                }
            }
            (true, false) => Expectation::Stdout(normalize(&read(&stdout_p))),
            (false, true) => match parse_diagnostic_code(&read(&error_p)) {
                Ok(exp) => exp,
                Err(p) => {
                    err(format!("expected.error: {p}"));
                    continue;
                }
            },
        };

        let expected_warning = if warning_p.is_file() {
            match parse_diagnostic_code(&read(&warning_p)) {
                Ok(exp) => match exp {
                    Expectation::Error { code, line } => Some(WarningCheck { code, line }),
                    _ => unreachable!(),
                },
                Err(p) => {
                    err(format!("expected.warning: {p}"));
                    continue;
                }
            }
        } else {
            None
        };

        cases.push(Case {
            name,
            dir,
            expectation,
            milestone,
            until,
            expected_warning,
            mode,
            arch,
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

fn parse_diagnostic_code(text: &str) -> Result<Expectation, String> {
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

#[allow(clippy::type_complexity)]
fn parse_case_toml(
    p: &Path,
) -> Result<(Option<u32>, Option<u32>, RunMode, Option<String>), String> {
    if !p.is_file() {
        return Ok((None, None, RunMode::Run, None));
    }
    let mut milestone = None;
    let mut until = None;
    let mut mode = RunMode::Run;
    let mut arch = None;
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
            milestone = Some(parse_milestone(v)?);
            continue;
        }
        if let Some(v) = l.strip_prefix("until") {
            let v = v
                .trim_start()
                .strip_prefix('=')
                .ok_or("expected `until = \"MN\"`")?;
            let v = v.trim().trim_matches('"');
            until = Some(parse_milestone(v)?);
            continue;
        }
        if let Some(v) = l.strip_prefix("mode") {
            let v = v
                .trim_start()
                .strip_prefix('=')
                .ok_or("expected `mode = \"run\"|\"test\"|\"build\"`")?;
            let v = v.trim().trim_matches('"');
            mode = match v {
                "run" => RunMode::Run,
                "test" => RunMode::Test,
                "build" => RunMode::Build,
                "audit" => RunMode::Audit,
                "gen" => RunMode::Gen,
                "repro" => RunMode::Repro,
                "image" => RunMode::Image,
                other => return Err(format!("unrecognized mode `{other}`")),
            };
            continue;
        }
        if let Some(v) = l.strip_prefix("arch") {
            let v = v
                .trim_start()
                .strip_prefix('=')
                .ok_or("expected `arch = \"amd64\"|\"arm64\"`")?;
            let v = v.trim().trim_matches('"');
            match v {
                "amd64" | "arm64" => arch = Some(v.to_string()),
                other => return Err(format!("unrecognized arch `{other}`")),
            }
            continue;
        }
        return Err(format!(
            "unrecognized key in `{l}` (only `milestone`, `until`, `mode`, and `arch` are allowed)"
        ));
    }
    if arch.is_some() && mode != RunMode::Image {
        return Err("`arch` is only valid with `mode = \"image\"`".into());
    }
    Ok((milestone, until, mode, arch))
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
    if let (Some(until), Some(cur)) = (case.until, current_milestone) {
        if cur > until {
            return Outcome::Skip(format!("only valid through M{until}, running at M{cur}"));
        }
    }

    match current_milestone {
        Some(1) => return check_m1_syntax(case, keelc, current_milestone),
        Some(2) => return check_m2_semantics(case, keelc, current_milestone),
        _ => {}
    }

    if case.mode == RunMode::Test && current_milestone < Some(4) {
        return Outcome::Skip(format!(
            "requires M4 test mode, running at M{}",
            current_milestone.unwrap_or(0)
        ));
    }
    if case.mode == RunMode::Build && current_milestone < Some(4) {
        return Outcome::Skip(format!(
            "requires M4 build mode, running at M{}",
            current_milestone.unwrap_or(0)
        ));
    }
    if case.mode == RunMode::Audit && current_milestone < Some(7) {
        return Outcome::Skip(format!(
            "requires M7 audit mode, running at M{}",
            current_milestone.unwrap_or(0)
        ));
    }
    if case.mode == RunMode::Gen {
        if current_milestone < Some(7) {
            return Outcome::Skip(format!(
                "requires M7 gen mode, running at M{}",
                current_milestone.unwrap_or(0)
            ));
        }
        return run_gen_case(case, keelc, current_milestone);
    }
    if case.mode == RunMode::Repro {
        if current_milestone < Some(7) {
            return Outcome::Skip(format!(
                "requires M7 repro mode, running at M{}",
                current_milestone.unwrap_or(0)
            ));
        }
        return run_repro_case(case, keelc, current_milestone);
    }
    if case.mode == RunMode::Image {
        if current_milestone < Some(9) {
            return Outcome::Skip(format!(
                "requires M9 image mode, running at M{}",
                current_milestone.unwrap_or(0)
            ));
        }
        return run_image_case(case, keelc, current_milestone);
    }

    let command = match case.mode {
        RunMode::Test => "test",
        RunMode::Build => "build",
        RunMode::Run => "run",
        RunMode::Audit => "audit",
        // Gen/Repro/Image return above; the match stays exhaustive without a panic.
        RunMode::Gen | RunMode::Repro | RunMode::Image => {
            return Outcome::Fail("internal: gen/repro/image fell through".into())
        }
    };
    let out = match invoke_keelc(case, keelc, command, current_milestone) {
        Ok(o) => o,
        Err(e) => return Outcome::Fail(format!("could not invoke `{keelc}`: {e}")),
    };

    // In build mode we compare the stdout of the *produced binary*, not the
    // compiler itself — unless the expectation is BuildOnly.
    let (out, stdout, stderr) =
        if case.mode == RunMode::Build && case.expectation != Expectation::BuildOnly {
            if !out.status.success() {
                let stderr = String::from_utf8_lossy(&out.stderr).to_string();
                return Outcome::Fail(format!(
                    "expected successful build, keelc exited {:?}\n--- stderr ---\n{stderr}",
                    out.status.code()
                ));
            }
            let binary = case
                .dir
                .join(format!("main{}", std::env::consts::EXE_SUFFIX));
            let binary = fs::canonicalize(&binary).unwrap_or_else(|_| binary.clone());
            let run = match Command::new(&binary)
                .current_dir(&case.dir)
                .stdin(Stdio::null())
                .output()
            {
                Ok(o) => o,
                Err(e) => {
                    remove_build_artifacts(&binary);
                    return Outcome::Fail(format!(
                        "could not run built binary `{}`: {e}",
                        binary.display()
                    ));
                }
            };
            remove_build_artifacts(&binary);
            let stdout = normalize(&String::from_utf8_lossy(&run.stdout));
            let stderr = String::from_utf8_lossy(&run.stderr).to_string();
            (run, stdout, stderr)
        } else {
            let stdout = normalize(&String::from_utf8_lossy(&out.stdout));
            let stderr = String::from_utf8_lossy(&out.stderr).to_string();
            (out, stdout, stderr)
        };

    match &case.expectation {
        Expectation::BuildOnly => {
            if !out.status.success() {
                return Outcome::Fail(format!(
                    "expected successful build, keelc exited {:?}\n--- stderr ---\n{stderr}",
                    out.status.code()
                ));
            }
            Outcome::Pass
        }
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
            if let Some(warning) = &case.expected_warning {
                if let Some(fail) = check_warning(&stderr, warning) {
                    return Outcome::Fail(fail);
                }
            }
            Outcome::Pass
        }
        Expectation::Error { code, line } => check_expected_error(
            &out.status,
            &stderr,
            code,
            line,
            &format!("expected compile error {code}, but program ran successfully"),
        ),
    }
}

fn milestone_args(cmd: &mut Command, current_milestone: Option<u32>) {
    if let Some(m) = current_milestone {
        cmd.arg("--milestone").arg(format!("M{m}"));
    }
}

/// `mode = "gen"`: `keelc gen schema.proto` emits Keel to stdout. Accept-cases
/// compare stdout, then assert the output round-trips `keel fmt` (spec ch17).
fn run_gen_case(case: &Case, keelc: &str, current_milestone: Option<u32>) -> Outcome {
    let mut cmd = Command::new(keelc);
    cmd.arg("gen").arg("schema.proto").current_dir(&case.dir);
    milestone_args(&mut cmd, current_milestone);
    let out = match cmd.stdin(Stdio::null()).output() {
        Ok(o) => o,
        Err(e) => return Outcome::Fail(format!("could not invoke `{keelc}`: {e}")),
    };
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    let want = match &case.expectation {
        Expectation::Error { code, line } => {
            return check_expected_error(
                &out.status,
                &stderr,
                code,
                line,
                &format!("expected gen error {code}, but generation succeeded"),
            )
        }
        Expectation::Stdout(want) => want,
        Expectation::BuildOnly => {
            return Outcome::Fail("gen mode needs expected.stdout or expected.error".into())
        }
    };

    if !out.status.success() {
        return Outcome::Fail(format!(
            "expected successful gen, keelc exited {:?}\n--- stderr ---\n{stderr}",
            out.status.code()
        ));
    }
    let stdout = normalize(&String::from_utf8_lossy(&out.stdout));
    if &stdout != want {
        return Outcome::Fail(diff("stdout", want, &stdout));
    }

    // Round-trip: the generated source must be `keel fmt`-idempotent (ch17 §17.2).
    let generated = case.dir.join("generated.keel");
    if let Err(e) = fs::write(&generated, &stdout) {
        return Outcome::Fail(format!("could not write {}: {e}", generated.display()));
    }
    let mut fmt = Command::new(keelc);
    fmt.arg("fmt").arg("generated.keel").current_dir(&case.dir);
    milestone_args(&mut fmt, current_milestone);
    let fmt_out = fmt.stdin(Stdio::null()).output();
    let _ = fs::remove_file(&generated);
    let fmt_out = match fmt_out {
        Ok(o) => o,
        Err(e) => return Outcome::Fail(format!("could not invoke `{keelc} fmt`: {e}")),
    };
    if !fmt_out.status.success() {
        return Outcome::Fail(format!(
            "generated source did not parse for `keel fmt`\n--- stderr ---\n{}",
            String::from_utf8_lossy(&fmt_out.stderr)
        ));
    }
    let reformatted = normalize(&String::from_utf8_lossy(&fmt_out.stdout));
    if reformatted != stdout {
        return Outcome::Fail(diff(
            "generated source is not `keel fmt`-idempotent",
            &stdout,
            &reformatted,
        ));
    }
    Outcome::Pass
}

/// `mode = "repro"`: two clean `keelc build`s must be byte-identical, then one
/// binary is run and its stdout matched (spec ch18, hermetic builds).
/// Remove a `keel build` output and its up-to-date stamp (the driver's build
/// cache, keelc-driver/src/build_cache.rs): checked-in case directories must
/// stay clean, and every conformance build must be a real build, never a
/// cached no-op.
fn remove_build_artifacts(binary: &Path) {
    let _ = fs::remove_file(binary);
    if let (Some(dir), Some(name)) = (binary.parent(), binary.file_name().and_then(|n| n.to_str()))
    {
        let _ = fs::remove_file(dir.join(format!(".{name}.keelstamp")));
    }
}

fn run_repro_case(case: &Case, keelc: &str, current_milestone: Option<u32>) -> Outcome {
    let exe = format!("main{}", std::env::consts::EXE_SUFFIX);
    let binary = case.dir.join(&exe);

    let build = |label: &str| -> Result<Vec<u8>, Outcome> {
        let mut cmd = Command::new(keelc);
        cmd.arg("build").arg("main.keel").current_dir(&case.dir);
        milestone_args(&mut cmd, current_milestone);
        let out = cmd
            .stdin(Stdio::null())
            .output()
            .map_err(|e| Outcome::Fail(format!("could not invoke `{keelc}`: {e}")))?;
        if !out.status.success() {
            return Err(Outcome::Fail(format!(
                "expected successful build ({label}), keelc exited {:?}\n--- stderr ---\n{}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        fs::read(&binary).map_err(|e| Outcome::Fail(format!("could not read built binary: {e}")))
    };

    // Strip any leftover artifacts so build 1 is real, not a cached no-op.
    remove_build_artifacts(&binary);
    let first = match build("build 1") {
        Ok(bytes) => bytes,
        Err(o) => return o,
    };
    // Build 2 must also be a real build: the byte-identical contract is about
    // rebuilding (spec ch18), not about the driver's up-to-date cutoff.
    remove_build_artifacts(&binary);
    let second = match build("build 2") {
        Ok(bytes) => bytes,
        Err(o) => {
            remove_build_artifacts(&binary);
            return o;
        }
    };
    if first != second {
        remove_build_artifacts(&binary);
        return Outcome::Fail(format!(
            "two clean builds are not byte-identical ({} vs {} bytes)",
            first.len(),
            second.len()
        ));
    }

    // The binary from build 2 is still on disk; run it for the stdout check.
    // Canonicalize so the path is valid once `current_dir` is the case dir.
    let exe_path = fs::canonicalize(&binary).unwrap_or_else(|_| binary.clone());
    let run = Command::new(&exe_path)
        .current_dir(&case.dir)
        .stdin(Stdio::null())
        .output();
    remove_build_artifacts(&binary);
    let run = match run {
        Ok(o) => o,
        Err(e) => return Outcome::Fail(format!("could not run built binary: {e}")),
    };
    let stdout = normalize(&String::from_utf8_lossy(&run.stdout));
    match &case.expectation {
        Expectation::BuildOnly => Outcome::Pass,
        Expectation::Stdout(want) => {
            if &stdout == want {
                Outcome::Pass
            } else {
                Outcome::Fail(diff("stdout", want, &stdout))
            }
        }
        Expectation::Error { .. } => {
            Outcome::Fail("repro mode does not support expected.error".into())
        }
    }
}

/// `mode = "image"`: two clean `keelc build --image` runs must produce a
/// byte-identical OCI Image Layout, which must also be structurally valid
/// per spec ch19 / KDR-0107 (one layer, one `diff_ids` entry). Reproducibility
/// is the only expectation supported; `expected.stdout`/`expected.error` do
/// not apply (there is no running the image outside a container runtime).
fn run_image_case(case: &Case, keelc: &str, current_milestone: Option<u32>) -> Outcome {
    if case.expectation != Expectation::BuildOnly {
        return Outcome::Fail(
            "image mode only supports BuildOnly (no expected.stdout/error)".into(),
        );
    }
    let out1 = case.dir.join("image1.oci");
    let out2 = case.dir.join("image2.oci");
    let cleanup = || {
        let _ = fs::remove_dir_all(&out1);
        let _ = fs::remove_dir_all(&out2);
    };

    // `-o` is a bare, case-relative name: the child's cwd is already
    // `case.dir` (`current_dir` below), so a full path here would resolve
    // twice and nest under the case directory.
    let build = |label: &str, out_name: &str| -> Result<(), Outcome> {
        let mut cmd = Command::new(keelc);
        cmd.arg("build")
            .arg("main.keel")
            .arg("--image")
            .arg("-o")
            .arg(out_name)
            .current_dir(&case.dir);
        if let Some(arch) = &case.arch {
            cmd.arg("--arch").arg(arch);
        }
        milestone_args(&mut cmd, current_milestone);
        let out = cmd
            .stdin(Stdio::null())
            .output()
            .map_err(|e| Outcome::Fail(format!("could not invoke `{keelc}`: {e}")))?;
        if !out.status.success() {
            return Err(Outcome::Fail(format!(
                "expected successful image build ({label}), keelc exited {:?}\n--- stderr ---\n{}",
                out.status.code(),
                String::from_utf8_lossy(&out.stderr)
            )));
        }
        Ok(())
    };

    if let Err(o) = build("build 1", "image1.oci") {
        cleanup();
        return o;
    }
    if let Err(o) = build("build 2", "image2.oci") {
        cleanup();
        return o;
    }

    let (files1, files2) = match (read_layout_files(&out1), read_layout_files(&out2)) {
        (Ok(a), Ok(b)) => (a, b),
        (Err(e), _) | (_, Err(e)) => {
            cleanup();
            return Outcome::Fail(e);
        }
    };
    if files1 != files2 {
        cleanup();
        return Outcome::Fail("two clean `--image` builds are not byte-identical".into());
    }

    let validated = validate_oci_layout(&out1, &files1, case.arch.as_deref());
    cleanup();
    match validated {
        Ok(()) => Outcome::Pass,
        Err(e) => Outcome::Fail(e),
    }
}

/// Recursively reads an OCI layout directory into a sorted `(relative path,
/// bytes)` list, so two layouts can be compared without depending on
/// filesystem iteration order (AGENTS.md hard rule 7).
fn read_layout_files(dir: &Path) -> Result<Vec<(String, Vec<u8>)>, String> {
    let mut out = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let abs = dir.join(&rel);
        let entries = fs::read_dir(&abs).map_err(|e| format!("could not read {abs:?}: {e}"))?;
        for entry in entries {
            let entry = entry.map_err(|e| format!("could not read entry in {abs:?}: {e}"))?;
            let rel_path = rel.join(entry.file_name());
            let file_type = entry
                .file_type()
                .map_err(|e| format!("could not stat {:?}: {e}", entry.path()))?;
            if file_type.is_dir() {
                stack.push(rel_path);
            } else {
                let bytes = fs::read(entry.path())
                    .map_err(|e| format!("could not read {:?}: {e}", entry.path()))?;
                out.push((rel_path.to_string_lossy().replace('\\', "/"), bytes));
            }
        }
    }
    out.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(out)
}

/// Structural validation per spec §19.3/§19.4: `oci-layout` marker and
/// `index.json` present; the chain `index -> manifest -> config` resolves;
/// exactly one layer in the manifest and exactly one `diff_ids` entry in the
/// config.
fn validate_oci_layout(
    dir: &Path,
    files: &[(String, Vec<u8>)],
    expected_arch: Option<&str>,
) -> Result<(), String> {
    if !files.iter().any(|(p, _)| p == "oci-layout") {
        return Err("image layout is missing the `oci-layout` marker file".into());
    }
    let index_bytes = &files
        .iter()
        .find(|(p, _)| p == "index.json")
        .ok_or("image layout is missing `index.json`")?
        .1;
    let index = json::parse(&String::from_utf8_lossy(index_bytes))?;
    let manifest_desc = index
        .get("manifests")
        .and_then(Value::as_array)
        .and_then(|m| m.first())
        .ok_or("index.json: missing manifests[0]")?;

    let manifest_bytes = read_descriptor(dir, manifest_desc, "index.json: manifests[0]")?;
    let manifest = json::parse(&String::from_utf8_lossy(&manifest_bytes))?;
    let layers = manifest
        .get("layers")
        .and_then(Value::as_array)
        .ok_or("manifest: missing `layers` array")?;
    if layers.len() != 1 {
        return Err(format!(
            "manifest must list exactly one layer (spec §19.3), found {}",
            layers.len()
        ));
    }
    read_descriptor(dir, &layers[0], "manifest: layers[0]")?;

    let config_desc = manifest.get("config").ok_or("manifest: missing `config`")?;
    let config_bytes = read_descriptor(dir, config_desc, "manifest: config")?;
    let config = json::parse(&String::from_utf8_lossy(&config_bytes))?;
    let diff_ids = config
        .get("rootfs")
        .and_then(|r| r.get("diff_ids"))
        .and_then(Value::as_array)
        .ok_or("config: missing rootfs.diff_ids")?;
    if diff_ids.len() != 1 {
        return Err(format!(
            "config rootfs.diff_ids must have exactly one entry, found {}",
            diff_ids.len()
        ));
    }
    // Spec §19.2: the config's declared architecture must match the requested
    // `--arch`. Only checked when the case pins one; the default-arch case
    // asserts only reproducibility and structure.
    if let Some(want) = expected_arch {
        let got = config
            .get("architecture")
            .and_then(Value::as_str)
            .ok_or("config: missing `architecture`")?;
        if got != want {
            return Err(format!(
                "config architecture `{got}` does not match requested --arch `{want}` (spec §19.2)"
            ));
        }
    }
    Ok(())
}

/// Reads the blob a descriptor `{digest, size}` references and checks its
/// byte length against the declared `size` — the descriptor's own
/// self-consistency check, not just that the file exists.
fn read_descriptor(dir: &Path, desc: &Value, what: &str) -> Result<Vec<u8>, String> {
    let digest = desc
        .get("digest")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{what}: missing digest"))?;
    let size = desc
        .get("size")
        .and_then(Value::as_u64)
        .ok_or_else(|| format!("{what}: missing size"))?;
    let (algo, hex) = digest
        .split_once(':')
        .ok_or_else(|| format!("{what}: malformed digest `{digest}`"))?;
    if algo != "sha256" {
        return Err(format!("{what}: unsupported digest algorithm `{algo}`"));
    }
    let bytes = fs::read(dir.join("blobs/sha256").join(hex))
        .map_err(|e| format!("{what}: could not read blob {digest}: {e}"))?;
    if bytes.len() as u64 != size {
        return Err(format!(
            "{what}: descriptor size {size} does not match blob length {}",
            bytes.len()
        ));
    }
    Ok(bytes)
}

fn check_m2_semantics(case: &Case, keelc: &str, current_milestone: Option<u32>) -> Outcome {
    let out = match invoke_keelc(case, keelc, "check", current_milestone) {
        Ok(o) => o,
        Err(e) => return Outcome::Fail(format!("could not invoke `{keelc}`: {e}")),
    };
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    match &case.expectation {
        Expectation::BuildOnly | Expectation::Stdout(_) => {
            if out.status.success() {
                if let Some(warning) = &case.expected_warning {
                    if let Some(fail) = check_warning(&stderr, warning) {
                        return Outcome::Fail(fail);
                    }
                }
                Outcome::Pass
            } else {
                Outcome::Fail(format!(
                    "expected successful M2 semantic check, keelc exited {:?}\n--- stderr ---\n{stderr}",
                    out.status.code()
                ))
            }
        }
        Expectation::Error { code, line } => check_expected_error(
            &out.status,
            &stderr,
            code,
            line,
            &format!("expected compile error {code}, but semantic check succeeded"),
        ),
    }
}

fn check_m1_syntax(case: &Case, keelc: &str, current_milestone: Option<u32>) -> Outcome {
    let out = match invoke_keelc(case, keelc, "check", current_milestone) {
        Ok(o) => o,
        Err(e) => return Outcome::Fail(format!("could not invoke `{keelc}`: {e}")),
    };
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();

    match &case.expectation {
        Expectation::BuildOnly | Expectation::Stdout(_) => {
            if out.status.success() {
                Outcome::Pass
            } else {
                Outcome::Fail(format!(
                    "expected successful M1 syntax check, keelc exited {:?}\n--- stderr ---\n{stderr}",
                    out.status.code()
                ))
            }
        }
        Expectation::Error { code, line } if is_m1_syntax_code(code) => check_expected_error(
            &out.status,
            &stderr,
            code,
            line,
            &format!("expected M1 syntax diagnostic {code}, but check succeeded"),
        ),
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

fn check_warning(stderr: &str, warning: &WarningCheck) -> Option<String> {
    if !stderr.contains(&warning.code) {
        return Some(format!(
            "expected warning {} in stderr\n--- stderr ---\n{stderr}",
            warning.code
        ));
    }
    if let Some(n) = warning.line {
        let needle = format!("main.keel:{n}");
        if !stderr.contains(&needle) {
            return Some(format!(
                "expected warning span at {needle}\n--- stderr ---\n{stderr}"
            ));
        }
    }
    None
}

fn check_expected_error(
    status: &std::process::ExitStatus,
    stderr: &str,
    code: &str,
    line: &Option<u32>,
    success_msg: &str,
) -> Outcome {
    if status.success() {
        return Outcome::Fail(success_msg.to_string());
    }
    if !stderr.contains(code) {
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

fn invoke_keelc(
    case: &Case,
    keelc: &str,
    command: &str,
    current_milestone: Option<u32>,
) -> std::io::Result<std::process::Output> {
    let mut cmd = Command::new(keelc);
    cmd.arg(command).arg("main.keel").current_dir(&case.dir);
    if let Some(milestone) = current_milestone {
        cmd.arg("--milestone").arg(format!("M{milestone}"));
    }
    cmd.output()
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

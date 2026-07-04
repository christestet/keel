//! Keel CLI driver: check, run, fmt, and build Keel Core source files.

mod build_cache;
mod gen;
mod manifest;

use keelc_ast::pretty::pretty_print;
use keelc_diag::{Diagnostic, Severity};
use keelc_parse::parse_with_milestone;
use keelc_span::{LineIndex, SourceId};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

/// The highest implemented Core milestone (M7 exit). `keel lsp` and file
/// commands both default here so a developer trying Keel gets the full
/// current language without knowing the milestone system exists; `--milestone
/// M<N>` remains available on file commands for conformance/regression work
/// against an earlier milestone gate.
const LATEST_MILESTONE: u32 = 7;

pub fn main() -> ExitCode {
    let mut args = env::args_os();
    let program = args.next();

    let Some(command) = args.next() else {
        usage();
        return ExitCode::from(2);
    };

    if matches!(command.as_os_str().to_str(), Some("--version" | "-V")) {
        println!("{}", version_line(program.as_deref()));
        return ExitCode::SUCCESS;
    }

    if command.as_os_str().to_str() == Some("lsp") {
        return run_lsp();
    }

    let Some(path) = args.next() else {
        usage();
        return ExitCode::from(2);
    };
    let mut milestone = LATEST_MILESTONE;
    while let Some(arg) = args.next() {
        if arg != OsStr::new("--milestone") {
            usage();
            return ExitCode::from(2);
        }
        let Some(value) = args.next() else {
            usage();
            return ExitCode::from(2);
        };
        let Some(parsed) = parse_milestone(&value) else {
            usage();
            return ExitCode::from(2);
        };
        milestone = parsed;
    }

    let path = Path::new(&path);
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("could not read {}: {err}", path.display());
            return ExitCode::from(2);
        }
    };

    match command.as_os_str().to_str() {
        Some("fmt") => return fmt_file(path, &text, milestone),
        Some("audit") => return audit_workspace(path, milestone),
        Some("gen") => return gen_schema(path, &text),
        Some("check" | "run" | "build" | "test") => {}
        _ => {
            eprintln!(
                "unsupported command `{}`; keel supports `build|run|fmt|test|check|audit|gen <file>`",
                command.to_string_lossy()
            );
            return ExitCode::from(2);
        }
    }

    // M7 package + capability enforcement (spec ch06/ch11). A no-op for an
    // implicit single-file package (no adjacent keel.toml).
    let manifest_diagnostics = manifest::check_workspace(path, milestone);
    if !manifest_diagnostics.is_empty() {
        for (code, message) in &manifest_diagnostics {
            eprintln!("error[{code}]: {message}");
        }
        return ExitCode::FAILURE;
    }

    // Up-to-date cutoff (KDR-0019 incremental budget): when nothing that
    // feeds the binary changed since the last clean `keel build`, skip the
    // frontend and `go build` entirely. Runs after the manifest gate so an
    // invalid workspace still fails every time.
    let build_stamp = if command.as_os_str().to_str() == Some("build") {
        build_cache::BuildStamp::compute(path, &text, milestone)
    } else {
        None
    };
    if let Some(stamp) = &build_stamp {
        if stamp.is_up_to_date() {
            return ExitCode::SUCCESS;
        }
    }

    let db = keelc_query::QueryDatabase::default();
    let source = keelc_query::SourceFile::new(&db, text.clone(), milestone);
    let Ok(clean) = emit_check_diagnostics(path, &text, &db, source) else {
        return ExitCode::FAILURE;
    };

    match command.as_os_str().to_str() {
        Some("check") => ExitCode::SUCCESS,
        Some("run") => match query_go_source(&db, source, false) {
            Ok(go_source) => run_go(go_source, "keelc-go"),
            Err(code) => code,
        },
        Some("test") => match query_go_source(&db, source, true) {
            Ok(go_source) => run_go(go_source, "keelc-go-tests"),
            Err(code) => code,
        },
        Some("build") => match query_go_source(&db, source, false) {
            Ok(go_source) => match build_go_source(path, &go_source) {
                Ok(()) => {
                    // Only a diagnostic-free build may be replayed as a
                    // no-op: a cached skip prints nothing, so it must only
                    // stand in for builds that printed nothing.
                    if clean {
                        if let Some(stamp) = &build_stamp {
                            stamp.record();
                        }
                    }
                    ExitCode::SUCCESS
                }
                Err(code) => code,
            },
            Err(code) => code,
        },
        _ => ExitCode::SUCCESS,
    }
}

fn usage() {
    eprintln!(
        "usage: keel <build|run|fmt|test|check|audit> <file.keel> [--milestone M<N>]\n       keel gen <schema.proto>\n       keel lsp\n       keel --version\n\n--milestone defaults to the latest implemented milestone (M7); pass an\nearlier M<N> only to check conformance against that milestone's gate."
    );
}

/// The `--version` line: binary name (from argv[0], so `keel` and `keelc`
/// report as themselves), crate version, and the commit the release was built
/// from. `KEEL_BUILD_COMMIT` is set by the release build; source builds
/// honestly report `unknown`.
fn version_line(program: Option<&OsStr>) -> String {
    let name = program
        .map(Path::new)
        .and_then(Path::file_stem)
        .and_then(OsStr::to_str)
        .unwrap_or("keel");
    let commit = option_env!("KEEL_BUILD_COMMIT").unwrap_or("unknown");
    format!("{name} {} (commit {commit})", env!("CARGO_PKG_VERSION"))
}

/// `keel lsp`: run the M8 base LSP server over stdio (spec ch. 16, KDR-0103).
/// A long-lived daemon, not a file command — it takes no path argument and
/// reads/writes JSON-RPC frames on stdin/stdout until `exit`.
fn run_lsp() -> ExitCode {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut reader = stdin.lock();
    let mut writer = stdout.lock();
    match keelc_lsp::serve(&mut reader, &mut writer, LATEST_MILESTONE) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("keel lsp: {err}");
            ExitCode::FAILURE
        }
    }
}

/// `keel gen`: emit Keel source from a schema (spec ch17). The format is chosen
/// by extension; today only `.proto` is recognized.
fn gen_schema(path: &Path, text: &str) -> ExitCode {
    if path.extension().and_then(OsStr::to_str) != Some("proto") {
        eprintln!("keel gen: unrecognized schema extension; expected a `.proto` file");
        return ExitCode::from(2);
    }
    match gen::generate(text) {
        Ok(source) => {
            print!("{source}");
            ExitCode::SUCCESS
        }
        Err((code, message)) => {
            eprintln!("error[{code}]: {message}");
            ExitCode::FAILURE
        }
    }
}

/// `keel audit`: print the deterministic capability report (spec §11.5), or the
/// manifest diagnostics that block it.
fn audit_workspace(path: &Path, milestone: u32) -> ExitCode {
    match manifest::audit(path, milestone) {
        Ok(report) => {
            print!("{report}");
            ExitCode::SUCCESS
        }
        Err(diagnostics) => {
            for (code, message) in &diagnostics {
                eprintln!("error[{code}]: {message}");
            }
            ExitCode::FAILURE
        }
    }
}

fn fmt_file(path: &Path, text: &str, milestone: u32) -> ExitCode {
    let output = parse_with_milestone(SourceId::new(0), text, milestone);
    let has_error = output.diagnostics.iter().any(is_error);
    let index = LineIndex::new(text);
    for diagnostic in &output.diagnostics {
        emit_diagnostic(path, &index, diagnostic);
    }
    if has_error {
        return ExitCode::FAILURE;
    }
    print!("{}", pretty_print(&output.module));
    ExitCode::SUCCESS
}

/// Print check diagnostics. `Err(())` when any is an error; otherwise
/// `Ok(clean)`, where `clean` means zero diagnostics — warnings pass the
/// build but block the build-cache stamp (see the `build` arm).
fn emit_check_diagnostics(
    path: &Path,
    text: &str,
    db: &keelc_query::QueryDatabase,
    source: keelc_query::SourceFile,
) -> Result<bool, ()> {
    let diagnostics = keelc_query::check_diagnostics(db, source);
    let has_error = diagnostics.iter().any(is_error);
    let index = LineIndex::new(text);
    for diagnostic in diagnostics.iter() {
        emit_diagnostic(path, &index, diagnostic);
    }
    if has_error {
        Err(())
    } else {
        Ok(diagnostics.is_empty())
    }
}

fn build_go_source(source_path: &Path, go_source: &str) -> Result<(), ExitCode> {
    let temp_dir = env::temp_dir().join(format!("keelc-build-{}", std::process::id()));
    if let Err(err) = fs::create_dir_all(&temp_dir) {
        eprintln!(
            "could not create Go build directory {}: {err}",
            temp_dir.display()
        );
        return Err(ExitCode::from(2));
    }
    let _guard = TempDir(temp_dir.clone());

    let module_mode = write_go_project(&temp_dir, go_source)?;

    // Absolute so `-o` is correct even when we run `go build` from the module dir.
    let binary_path = build_cache::output_binary_path(source_path);

    let mut command = Command::new("go");
    // Hermetic build (spec ch18, KDR-0105): `-trimpath` strips the per-invocation
    // temp build path and `-buildvcs=false` keeps VCS metadata out of the binary,
    // so two clean builds of the same source are byte-identical.
    command
        .arg("build")
        .arg("-trimpath")
        .arg("-buildvcs=false")
        .arg("-o")
        .arg(&binary_path);
    if module_mode {
        command.arg(".").current_dir(&temp_dir);
    } else {
        command.arg(temp_dir.join("main.go"));
    }
    let output = match command.stdin(Stdio::null()).output() {
        Ok(output) => output,
        Err(err) => {
            eprintln!("could not invoke Go toolchain: {err}");
            return Err(ExitCode::from(2));
        }
    };

    if output.status.success() {
        Ok(())
    } else {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        Err(ExitCode::FAILURE)
    }
}

fn parse_milestone(value: &OsStr) -> Option<u32> {
    value.to_str()?.strip_prefix('M')?.parse().ok()
}

fn is_error(diagnostic: &Diagnostic) -> bool {
    diagnostic.severity == Severity::Error
}

fn emit_diagnostic(path: &Path, index: &LineIndex, diagnostic: &Diagnostic) {
    let severity = match diagnostic.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    let loc = index.line_col(diagnostic.span.start);
    let label = file_label(path);

    eprintln!("{severity}[{}]: {}", diagnostic.code, diagnostic.message);
    eprintln!("  --> {label}:{}:{}", loc.line, loc.column);
    if let Some(help) = &diagnostic.help {
        eprintln!("  help: {help}");
    }
}

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Write the generated Go into `temp_dir`. When the program imports an external
/// module (the SQLite driver, KDR-0042), also emit a `go.mod` and resolve
/// dependencies so `go build`/`go run` works in module mode. Returns whether the
/// directory is a Go module (callers then build the package `.` instead of the
/// lone file).
fn write_go_project(temp_dir: &Path, go_source: &str) -> Result<bool, ExitCode> {
    let go_file = temp_dir.join("main.go");
    if let Err(err) = fs::write(&go_file, go_source) {
        eprintln!(
            "could not write generated Go source {}: {err}",
            go_file.display()
        );
        return Err(ExitCode::from(2));
    }
    if !go_source.contains("modernc.org/sqlite") {
        return Ok(false);
    }
    if let Err(err) = fs::write(temp_dir.join("go.mod"), "module keelout\n\ngo 1.21\n") {
        eprintln!("could not write go.mod: {err}");
        return Err(ExitCode::from(2));
    }
    // `go mod tidy` reads main.go's imports, fetches the driver, writes go.sum.
    let tidy = Command::new("go")
        .arg("mod")
        .arg("tidy")
        .current_dir(temp_dir)
        .stdin(Stdio::null())
        .output();
    match tidy {
        Ok(output) if output.status.success() => Ok(true),
        Ok(output) => {
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
            eprintln!("could not resolve Go module dependencies (go mod tidy failed)");
            Err(ExitCode::FAILURE)
        }
        Err(err) => {
            eprintln!("could not invoke Go toolchain: {err}");
            Err(ExitCode::from(2))
        }
    }
}

fn run_go(go_source: String, temp_prefix: &str) -> ExitCode {
    let temp_dir = env::temp_dir().join(format!("{temp_prefix}-{}", std::process::id()));
    if let Err(err) = fs::create_dir_all(&temp_dir) {
        eprintln!(
            "could not create Go build directory {}: {err}",
            temp_dir.display()
        );
        return ExitCode::from(2);
    }
    let _guard = TempDir(temp_dir.clone());

    let module_mode = match write_go_project(&temp_dir, &go_source) {
        Ok(module_mode) => module_mode,
        Err(code) => return code,
    };

    let mut command = Command::new("go");
    command.arg("run");
    if module_mode {
        command.arg(".").current_dir(&temp_dir);
    } else {
        command.arg(temp_dir.join("main.go"));
    }
    let output = match command.stdin(Stdio::null()).output() {
        Ok(output) => output,
        Err(err) => {
            eprintln!("could not invoke Go toolchain: {err}");
            return ExitCode::from(2);
        }
    };

    print!("{}", String::from_utf8_lossy(&output.stdout));
    eprint!("{}", String::from_utf8_lossy(&output.stderr));
    if output.status.success() {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}

fn query_go_source(
    db: &keelc_query::QueryDatabase,
    source: keelc_query::SourceFile,
    tests: bool,
) -> Result<String, ExitCode> {
    let emitted = if tests {
        keelc_query::go_test_source(db, source)
    } else {
        keelc_query::go_source(db, source)
    };
    emitted
        .as_ref()
        .clone()
        .map_err(|diagnostic| match diagnostic {
            keelc_query::EmitDiagnostic::Lowering(message) => {
                eprintln!("lowering error: {message}");
                ExitCode::FAILURE
            }
            keelc_query::EmitDiagnostic::Backend(message) => {
                eprintln!("backend error: {message}");
                ExitCode::FAILURE
            }
        })
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

#[cfg(test)]
mod version_tests {
    use super::version_line;
    use std::ffi::OsStr;

    #[test]
    fn version_line_uses_binary_name_and_crate_version() {
        let line = version_line(Some(OsStr::new("/usr/local/bin/keelc")));
        assert_eq!(
            line,
            format!("keelc {} (commit unknown)", env!("CARGO_PKG_VERSION"))
        );
        assert!(version_line(None).starts_with("keel "));
    }
}

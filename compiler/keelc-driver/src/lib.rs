//! Keel CLI driver: check, run, fmt, and build Keel Core source files.

mod manifest;

use keelc_ast::pretty::pretty_print;
use keelc_ast::Module;
use keelc_backend_go::{emit, emit_tests};
use keelc_diag::{Diagnostic, Severity};
use keelc_kir::lower::lower;
use keelc_parse::parse_with_milestone;
use keelc_resolve::{resolve, typecheck, TypecheckOutput};
use keelc_span::{LineIndex, SourceId};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

pub fn main() -> ExitCode {
    let mut args = env::args_os();
    let _program = args.next();

    let Some(command) = args.next() else {
        usage();
        return ExitCode::from(2);
    };
    let Some(path) = args.next() else {
        usage();
        return ExitCode::from(2);
    };
    let mut milestone = 1u32;
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
        Some("check" | "run" | "build" | "test") => {}
        _ => {
            eprintln!(
                "unsupported command `{}`; keel supports `build|run|fmt|test|check|audit <file>`",
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

    let output = parse_with_milestone(SourceId::new(0), &text, milestone);
    let mut diagnostics = output.diagnostics;
    let checked = typecheck(&output.module);
    if milestone >= 2 && !diagnostics.iter().any(is_error) {
        diagnostics.extend(resolve(&output.module).diagnostics);
        if !diagnostics.iter().any(is_error) {
            diagnostics.extend(checked.diagnostics.iter().cloned());
        }
    }
    diagnostics.sort_by(|left, right| {
        left.span
            .start
            .cmp(&right.span.start)
            .then_with(|| left.span.end.cmp(&right.span.end))
            .then_with(|| left.code.as_str().cmp(right.code.as_str()))
    });

    let has_error = diagnostics.iter().any(is_error);
    let index = LineIndex::new(&text);
    for diagnostic in &diagnostics {
        emit_diagnostic(path, &index, diagnostic);
    }

    if has_error {
        return ExitCode::FAILURE;
    }

    match command.as_os_str().to_str() {
        Some("run") => run_module(&output.module, &text, &checked),
        Some("test") => run_tests(&output.module, &text, &checked),
        Some("build") => build_module(&output.module, path, &text, &checked),
        _ => ExitCode::SUCCESS,
    }
}

fn usage() {
    eprintln!("usage: keel <build|run|fmt|test|check|audit> <file.keel> [--milestone M<N>]");
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

fn build_module(
    module: &Module,
    source_path: &Path,
    source: &str,
    checked: &TypecheckOutput,
) -> ExitCode {
    let go_source = match emit_go(module, source, checked, false) {
        Ok(source) => source,
        Err(code) => return code,
    };

    let temp_dir = env::temp_dir().join(format!("keelc-build-{}", std::process::id()));
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

    let binary_name = source_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("keel-out");
    let binary_name = format!("{binary_name}{}", std::env::consts::EXE_SUFFIX);
    let output_dir = source_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let binary_path = output_dir.join(&binary_name);
    // Absolute so `-o` is correct even when we run `go build` from the module dir.
    let binary_path = fs::canonicalize(output_dir)
        .map(|dir| dir.join(&binary_name))
        .unwrap_or(binary_path);

    let mut command = Command::new("go");
    command.arg("build").arg("-o").arg(&binary_path);
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

    if output.status.success() {
        ExitCode::SUCCESS
    } else {
        eprint!("{}", String::from_utf8_lossy(&output.stderr));
        ExitCode::FAILURE
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

fn run_module(module: &Module, source: &str, checked: &TypecheckOutput) -> ExitCode {
    let go_source = match emit_go(module, source, checked, false) {
        Ok(source) => source,
        Err(code) => return code,
    };
    run_go(go_source, "keelc-go")
}

fn run_tests(module: &Module, source: &str, checked: &TypecheckOutput) -> ExitCode {
    let go_source = match emit_go(module, source, checked, true) {
        Ok(source) => source,
        Err(code) => return code,
    };

    run_go(go_source, "keelc-go-tests")
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

fn emit_go(
    module: &Module,
    source: &str,
    checked: &TypecheckOutput,
    tests: bool,
) -> Result<String, ExitCode> {
    let kir_output = lower(module, source, &checked.types);
    if !kir_output.diagnostics.is_empty() {
        eprintln!("lowering error: {}", kir_output.diagnostics[0].message);
        return Err(ExitCode::FAILURE);
    }
    let go_source = match if tests {
        emit_tests(&kir_output.module)
    } else {
        emit(&kir_output.module)
    } {
        Ok(source) => source,
        Err(err) => {
            eprintln!("backend error: {err}");
            return Err(ExitCode::FAILURE);
        }
    };
    Ok(go_source)
}

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

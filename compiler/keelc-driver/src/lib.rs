//! Keel CLI driver: check, run, fmt, and build Keel Core source files.

use keelc_ast::pretty::pretty_print;
use keelc_ast::Module;
use keelc_backend_go::{emit, emit_tests};
use keelc_diag::{Diagnostic, Severity};
use keelc_kir::lower::lower;
use keelc_parse::parse_with_milestone;
use keelc_resolve::{resolve, typecheck};
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
        Some("check" | "run" | "build" | "test") => {}
        _ => {
            eprintln!(
                "unsupported command `{}`; keel supports `build|run|fmt|test|check <file>`",
                command.to_string_lossy()
            );
            return ExitCode::from(2);
        }
    }

    let output = parse_with_milestone(SourceId::new(0), &text, milestone);
    let mut diagnostics = output.diagnostics;
    if milestone >= 2 && !diagnostics.iter().any(is_error) {
        diagnostics.extend(resolve(&output.module).diagnostics);
        if !diagnostics.iter().any(is_error) {
            diagnostics.extend(typecheck(&output.module).diagnostics);
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
        Some("run") => run_module(&output.module, &text),
        Some("test") => run_tests(&output.module, &text),
        Some("build") => build_module(&output.module, path, &text),
        _ => ExitCode::SUCCESS,
    }
}

fn usage() {
    eprintln!("usage: keel <build|run|fmt|test|check> <file.keel> [--milestone M<N>]");
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

fn build_module(module: &Module, source_path: &Path, source: &str) -> ExitCode {
    let go_source = match emit_go(module, source, false) {
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

    let go_file = temp_dir.join("main.go");
    if let Err(err) = fs::write(&go_file, go_source) {
        eprintln!(
            "could not write generated Go source {}: {err}",
            go_file.display()
        );
        return ExitCode::from(2);
    }

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

    let output = match Command::new("go")
        .arg("build")
        .arg("-o")
        .arg(&binary_path)
        .arg(&go_file)
        .stdin(Stdio::null())
        .output()
    {
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

fn run_module(module: &Module, source: &str) -> ExitCode {
    let go_source = match emit_go(module, source, false) {
        Ok(source) => source,
        Err(code) => return code,
    };
    let temp_dir = env::temp_dir().join(format!("keelc-go-{}", std::process::id()));
    if let Err(err) = fs::create_dir_all(&temp_dir) {
        eprintln!(
            "could not create Go build directory {}: {err}",
            temp_dir.display()
        );
        return ExitCode::from(2);
    }
    let _guard = TempDir(temp_dir.clone());

    let go_file = temp_dir.join("main.go");
    if let Err(err) = fs::write(&go_file, go_source) {
        eprintln!(
            "could not write generated Go source {}: {err}",
            go_file.display()
        );
        return ExitCode::from(2);
    }

    let output = match Command::new("go")
        .arg("run")
        .arg(&go_file)
        .stdin(Stdio::null())
        .output()
    {
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

fn run_tests(module: &Module, source: &str) -> ExitCode {
    let go_source = match emit_go(module, source, true) {
        Ok(source) => source,
        Err(code) => return code,
    };

    let temp_dir = env::temp_dir().join(format!("keelc-go-tests-{}", std::process::id()));
    if let Err(err) = fs::create_dir_all(&temp_dir) {
        eprintln!(
            "could not create Go build directory {}: {err}",
            temp_dir.display()
        );
        return ExitCode::from(2);
    }
    let _guard = TempDir(temp_dir.clone());

    let go_file = temp_dir.join("main.go");
    if let Err(err) = fs::write(&go_file, go_source) {
        eprintln!(
            "could not write generated Go source {}: {err}",
            go_file.display()
        );
        return ExitCode::from(2);
    }

    let output = match Command::new("go")
        .arg("run")
        .arg(&go_file)
        .stdin(Stdio::null())
        .output()
    {
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

fn emit_go(module: &Module, source: &str, tests: bool) -> Result<String, ExitCode> {
    let kir_output = lower(module, source);
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

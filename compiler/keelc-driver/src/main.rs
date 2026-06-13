//! Minimal keelc driver: read one source file, run frontend checks, print diagnostics.

use keelc_ast::Module;
use keelc_backend_go::emit;
use keelc_diag::{Diagnostic, Severity};
use keelc_parse::parse;
use keelc_resolve::{resolve, typecheck};
use keelc_span::{line_col, SourceId};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};

fn main() -> ExitCode {
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

    if command != OsStr::new("check") && command != OsStr::new("run") {
        eprintln!(
            "unsupported command `{}`; keelc supports `check <file>` and `run <file>`",
            command.to_string_lossy()
        );
        return ExitCode::from(2);
    }

    let path = Path::new(&path);
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) => {
            eprintln!("could not read {}: {err}", path.display());
            return ExitCode::from(2);
        }
    };

    let output = parse(SourceId::new(0), &text);
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
    for diagnostic in &diagnostics {
        emit_diagnostic(path, &text, diagnostic);
    }

    if has_error {
        ExitCode::FAILURE
    } else if command == OsStr::new("run") {
        run_module(&output.module)
    } else {
        ExitCode::SUCCESS
    }
}

fn usage() {
    eprintln!("usage: keelc check <main.keel> [--milestone M<N>]");
}

fn parse_milestone(value: &OsStr) -> Option<u32> {
    value.to_str()?.strip_prefix('M')?.parse().ok()
}

fn is_error(diagnostic: &Diagnostic) -> bool {
    diagnostic.severity == Severity::Error
}

fn emit_diagnostic(path: &Path, source: &str, diagnostic: &Diagnostic) {
    let severity = match diagnostic.severity {
        Severity::Error => "error",
        Severity::Warning => "warning",
    };
    let loc = line_col(source, diagnostic.span.start);
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

fn run_module(module: &Module) -> ExitCode {
    let go_source = match emit(module) {
        Ok(source) => source,
        Err(err) => {
            eprintln!("backend error: {err}");
            return ExitCode::FAILURE;
        }
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

    let go_cache = temp_dir.join("gocache");
    if let Err(err) = fs::create_dir_all(&go_cache) {
        eprintln!(
            "could not create Go cache directory {}: {err}",
            go_cache.display()
        );
        return ExitCode::from(2);
    }

    let output = match Command::new("go")
        .arg("run")
        .arg(&go_file)
        .env("GOCACHE", &go_cache)
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

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

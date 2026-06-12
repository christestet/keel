//! Minimal keelc driver: read one source file, run frontend checks, print diagnostics.

use keelc_diag::{Diagnostic, Severity};
use keelc_parse::parse;
use keelc_resolve::resolve;
use keelc_span::{line_col, SourceId};
use std::env;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

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

    if command != OsStr::new("check") {
        eprintln!(
            "unsupported command `{}`; keelc supports `check <file>`",
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

fn file_label(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| path.display().to_string())
}

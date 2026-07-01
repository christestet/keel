//! Salsa query core for M8 (KDR-0106).
//!
//! `keel check`/`run`/`test`/`build` (via `keelc-driver`) and `keel lsp` (via
//! `keelc-lsp`) both depend on this crate so they share one in-process query
//! graph instead of each driving their own copy of the pipeline. Living in a
//! crate of its own — rather than inside `keelc-driver` — avoids a dependency
//! cycle: `keelc-driver` depends on `keelc-lsp` to implement the `keel lsp`
//! subcommand, and `keelc-lsp` depends on this crate for parse/check queries.
//! KDR-0103 anticipates this: "the query surface may be exposed from
//! `keelc-driver` or moved into a separately justified compiler crate."
//!
//! Query functions remain pure and side-effect free; filesystem and process
//! work stays in the driver/server callers.

use keelc_diag::{Diagnostic, Severity};
use keelc_kir::lower::{lower, LowerOutput};
use keelc_parse::{parse_with_milestone, ParseOutput};
use keelc_resolve::{resolve, typecheck, ResolveOutput, TypecheckOutput};
use keelc_span::SourceId;
use std::sync::Arc;

pub type QueryDatabase = salsa::DatabaseImpl;

#[salsa::input]
pub struct SourceFile {
    #[returns(deref)]
    text: String,
    #[returns(copy)]
    milestone: u32,
}

#[salsa::tracked(returns(clone))]
pub fn parsed_module(db: &dyn salsa::Database, source: SourceFile) -> Arc<ParseOutput> {
    Arc::new(parse_with_milestone(
        SourceId::new(0),
        source.text(db),
        source.milestone(db),
    ))
}

#[salsa::tracked(returns(clone))]
pub fn resolved_module(db: &dyn salsa::Database, source: SourceFile) -> Arc<ResolveOutput> {
    let parsed = parsed_module(db, source);
    Arc::new(resolve(&parsed.module))
}

#[salsa::tracked(returns(clone))]
pub fn typechecked_module(db: &dyn salsa::Database, source: SourceFile) -> Arc<TypecheckOutput> {
    let parsed = parsed_module(db, source);
    Arc::new(typecheck(&parsed.module))
}

#[salsa::tracked(returns(clone))]
pub fn lowered_module(db: &dyn salsa::Database, source: SourceFile) -> Arc<LowerOutput> {
    let parsed = parsed_module(db, source);
    let checked = typechecked_module(db, source);
    Arc::new(lower(&parsed.module, source.text(db), &checked.types))
}

#[salsa::tracked(returns(clone))]
pub fn go_source(
    db: &dyn salsa::Database,
    source: SourceFile,
) -> Arc<Result<String, EmitDiagnostic>> {
    let lowered = lowered_module(db, source);
    if let Some(diagnostic) = lowered.diagnostics.first() {
        return Arc::new(Err(EmitDiagnostic::Lowering(diagnostic.message.clone())));
    }
    Arc::new(
        keelc_backend_go::emit(&lowered.module)
            .map_err(|error| EmitDiagnostic::Backend(error.to_string())),
    )
}

#[salsa::tracked(returns(clone))]
pub fn go_test_source(
    db: &dyn salsa::Database,
    source: SourceFile,
) -> Arc<Result<String, EmitDiagnostic>> {
    let lowered = lowered_module(db, source);
    if let Some(diagnostic) = lowered.diagnostics.first() {
        return Arc::new(Err(EmitDiagnostic::Lowering(diagnostic.message.clone())));
    }
    Arc::new(
        keelc_backend_go::emit_tests(&lowered.module)
            .map_err(|error| EmitDiagnostic::Backend(error.to_string())),
    )
}

#[salsa::tracked(returns(clone))]
pub fn check_diagnostics(db: &dyn salsa::Database, source: SourceFile) -> Arc<Vec<Diagnostic>> {
    let parsed = parsed_module(db, source);
    let mut diagnostics = parsed.diagnostics.clone();

    if source.milestone(db) >= 2 && !diagnostics.iter().any(is_error) {
        diagnostics.extend(resolved_module(db, source).diagnostics.iter().cloned());
        if !diagnostics.iter().any(is_error) {
            diagnostics.extend(typechecked_module(db, source).diagnostics.iter().cloned());
        }
    }

    diagnostics.sort_by(|left, right| {
        left.span
            .start
            .cmp(&right.span.start)
            .then_with(|| left.span.end.cmp(&right.span.end))
            .then_with(|| left.code.as_str().cmp(right.code.as_str()))
    });
    Arc::new(diagnostics)
}

fn is_error(diagnostic: &Diagnostic) -> bool {
    diagnostic.severity == Severity::Error
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum EmitDiagnostic {
    Lowering(String),
    Backend(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use keelc_resolve::{resolve, typecheck};

    fn direct_diagnostics(source: &str, milestone: u32) -> Vec<Diagnostic> {
        let output = parse_with_milestone(SourceId::new(0), source, milestone);
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
        diagnostics
    }

    #[test]
    fn check_diagnostics_match_direct_pipeline() {
        let source = "fn main() -> Unit {\n    let x = 1.0 + 2\n}\n";
        let db = QueryDatabase::default();
        let file = SourceFile::new(&db, source.to_owned(), 7);

        assert_eq!(
            &*check_diagnostics(&db, file),
            &direct_diagnostics(source, 7)
        );
    }

    #[test]
    fn go_source_emits_from_checked_query_outputs() {
        let source = "fn main() -> Unit {\n    print(\"hello\")\n}\n";
        let db = QueryDatabase::default();
        let file = SourceFile::new(&db, source.to_owned(), 7);

        let go = go_source(&db, file).as_ref().clone().expect("Go source");
        assert!(go.contains("func main()"));
        assert!(go.contains("hello"));
    }
}

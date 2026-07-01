//! Query-backed frontend entry points for M8.
//!
//! The current database is driver-internal so the first implementation slice can
//! route `keel check` without changing crate boundaries. Query functions remain
//! pure and side-effect free; filesystem and process work stays in `lib.rs`.

use keelc_diag::{Diagnostic, Severity};
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
}

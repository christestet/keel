//! Maps `keelc_diag::Diagnostic` to the LSP `Diagnostic` shape (spec ch. 16 §16.2).

use crate::documents::Utf16Index;
use keelc_diag::Severity;
use lsp_types::{Diagnostic, DiagnosticSeverity, NumberOrString};

#[must_use]
pub fn to_lsp_diagnostics(diagnostics: &[keelc_diag::Diagnostic], text: &str) -> Vec<Diagnostic> {
    let index = Utf16Index::new(text);
    diagnostics
        .iter()
        .map(|diagnostic| {
            let mut message = diagnostic.message.clone();
            if let Some(help) = &diagnostic.help {
                message.push('\n');
                message.push_str(help);
            }
            Diagnostic {
                range: crate::documents::range(
                    text,
                    &index,
                    diagnostic.span.start,
                    diagnostic.span.end,
                ),
                severity: Some(match diagnostic.severity {
                    Severity::Error => DiagnosticSeverity::ERROR,
                    Severity::Warning => DiagnosticSeverity::WARNING,
                }),
                code: Some(NumberOrString::String(diagnostic.code.as_str().to_owned())),
                source: Some("keelc".to_owned()),
                message,
                ..Default::default()
            }
        })
        .collect()
}

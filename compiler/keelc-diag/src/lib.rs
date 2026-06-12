//! Diagnostic types and stable error-code registry for keelc.

use keelc_span::Span;
use std::fmt;

pub mod registry;

/// Stable public diagnostic code.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Code(&'static str);

impl Code {
    #[must_use]
    pub const fn new(raw: &'static str) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Severity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub code: Code,
    pub severity: Severity,
    pub message: String,
    pub span: Span,
    pub help: Option<String>,
}

impl Diagnostic {
    #[must_use]
    pub fn error(code: Code, span: Span, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Error,
            message: message.into(),
            span,
            help: None,
        }
    }

    #[must_use]
    pub fn warning(code: Code, span: Span, message: impl Into<String>) -> Self {
        Self {
            code,
            severity: Severity::Warning,
            message: message.into(),
            span,
            help: None,
        }
    }

    #[must_use]
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }
}

#[must_use]
pub fn is_registered(code: Code) -> bool {
    registry::ALL_CODES.iter().any(|entry| entry.code == code)
}

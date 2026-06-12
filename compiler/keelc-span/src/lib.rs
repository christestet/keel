//! Source file identifiers and spans for keelc diagnostics.

use std::fmt;

/// Stable identifier for a source file within one compiler invocation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SourceId(u32);

impl SourceId {
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn raw(self) -> u32 {
        self.0
    }
}

/// Half-open byte span within one source file.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct Span {
    pub source: SourceId,
    pub start: usize,
    pub end: usize,
}

impl Span {
    #[must_use]
    pub const fn new(source: SourceId, start: usize, end: usize) -> Self {
        Self { source, start, end }
    }

    #[must_use]
    pub const fn empty(source: SourceId, offset: usize) -> Self {
        Self {
            source,
            start: offset,
            end: offset,
        }
    }

    #[must_use]
    pub fn join(self, other: Self) -> Self {
        debug_assert_eq!(self.source, other.source);
        Self {
            source: self.source,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    #[must_use]
    pub const fn len(self) -> usize {
        self.end.saturating_sub(self.start)
    }

    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.start == self.end
    }
}

/// A value paired with its source span.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Spanned<T> {
    pub span: Span,
    pub value: T,
}

impl<T> Spanned<T> {
    #[must_use]
    pub const fn new(value: T, span: Span) -> Self {
        Self { span, value }
    }
}

/// Human-facing 1-based line/column location.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LineCol {
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for LineCol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// Compute a 1-based line/column pair for a byte offset.
///
/// Offsets past the end of the file map to the end position.
#[must_use]
pub fn line_col(source: &str, byte_offset: usize) -> LineCol {
    let target = byte_offset.min(source.len());
    let mut line = 1usize;
    let mut line_start = 0usize;

    for (idx, ch) in source.char_indices() {
        if idx >= target {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = idx + ch.len_utf8();
        }
    }

    LineCol {
        line,
        column: target.saturating_sub(line_start) + 1,
    }
}

#[cfg(test)]
mod tests {
    use super::{line_col, LineCol};

    #[test]
    fn maps_offsets_to_lines_and_columns() {
        let text = "a\nbc\n";

        assert_eq!(line_col(text, 0), LineCol { line: 1, column: 1 });
        assert_eq!(line_col(text, 2), LineCol { line: 2, column: 1 });
        assert_eq!(line_col(text, 4), LineCol { line: 2, column: 3 });
        assert_eq!(line_col(text, 99), LineCol { line: 3, column: 1 });
    }
}

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

/// Precomputed line-start index for fast repeated `byte_offset -> LineCol`
/// lookups.
///
/// Building the index is `O(bytes)`. Each lookup is `O(log lines)` via binary
/// search, which avoids the `O(diagnostics * bytes)` behaviour of calling
/// [`line_col`] once per diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineIndex {
    line_starts: Vec<usize>,
    source_len: usize,
}

impl LineIndex {
    #[must_use]
    pub fn new(source: &str) -> Self {
        let mut line_starts = vec![0];
        for (idx, ch) in source.char_indices() {
            if ch == '\n' {
                line_starts.push(idx + ch.len_utf8());
            }
        }
        Self {
            line_starts,
            source_len: source.len(),
        }
    }

    #[must_use]
    pub fn line_col(&self, byte_offset: usize) -> LineCol {
        let target = byte_offset.min(self.source_len);
        let line = self.line_starts.partition_point(|&start| start <= target);
        let line_start = self.line_starts[line - 1];
        LineCol {
            line,
            column: target.saturating_sub(line_start) + 1,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{LineCol, LineIndex};

    #[test]
    fn line_index_maps_offsets_to_lines_and_columns() {
        let text = "a\nbc\n";
        let index = LineIndex::new(text);

        assert_eq!(index.line_col(0), LineCol { line: 1, column: 1 });
        assert_eq!(index.line_col(2), LineCol { line: 2, column: 1 });
        assert_eq!(index.line_col(4), LineCol { line: 2, column: 3 });
        assert_eq!(index.line_col(99), LineCol { line: 3, column: 1 });
    }
}

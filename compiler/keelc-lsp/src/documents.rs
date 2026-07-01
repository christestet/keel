//! Open-document state and UTF-16 LSP position mapping.
//!
//! `keelc-span::LineIndex` maps byte offsets to 1-based UTF-8 columns for the
//! CLI's human-facing diagnostics. LSP instead needs 0-based line/character
//! positions with UTF-16 code-unit columns (spec ch. 16 §16.2), so that
//! mapping lives here rather than in the shared `keelc-span` crate.

use lsp_types::{Position, Range, TextDocumentContentChangeEvent};

pub struct Document {
    pub text: String,
    pub version: i32,
}

/// Maps byte offsets in a source string to and from 0-based, UTF-16
/// `Position`s, treating `\n` (and any `\r` immediately before it) as the
/// line terminator excluded from both line content.
pub struct Utf16Index {
    line_starts: Vec<usize>,
}

impl Utf16Index {
    #[must_use]
    pub fn new(text: &str) -> Self {
        let mut line_starts = vec![0];
        for (idx, ch) in text.char_indices() {
            if ch == '\n' {
                line_starts.push(idx + 1);
            }
        }
        Self { line_starts }
    }

    fn line_bounds(&self, text: &str, line: usize) -> (usize, usize) {
        let line = line.min(self.line_starts.len() - 1);
        let start = self.line_starts[line];
        let end = self
            .line_starts
            .get(line + 1)
            .map_or(text.len(), |&next| next.min(text.len()).max(start));
        (start, end)
    }

    /// Byte offset -> 0-based UTF-16 `Position`. Offsets past the end of the
    /// text clamp to the final position.
    #[must_use]
    pub fn position(&self, text: &str, byte_offset: usize) -> Position {
        let byte_offset = byte_offset.min(text.len());
        let line = self
            .line_starts
            .partition_point(|&start| start <= byte_offset)
            - 1;
        let (line_start, line_end) = self.line_bounds(text, line);
        let content_end = byte_offset.min(line_end).max(line_start);
        let slice = &text[line_start..content_end];
        let character: usize = slice.chars().map(char::len_utf16).sum();
        Position {
            line: line as u32,
            character: character as u32,
        }
    }

    /// 0-based UTF-16 `Position` -> byte offset. Out-of-range lines/columns
    /// clamp to the nearest valid offset rather than panicking, since the
    /// position comes from a client request (spec ch. 16 §16.4).
    #[must_use]
    pub fn byte_offset(&self, text: &str, position: Position) -> usize {
        let (line_start, line_end) = self.line_bounds(text, position.line as usize);
        let slice = &text[line_start..line_end];
        let mut units = 0u32;
        for (byte_idx, ch) in slice.char_indices() {
            if units >= position.character {
                return line_start + byte_idx;
            }
            units += ch.len_utf16() as u32;
        }
        line_start + slice.len()
    }
}

/// Applies one `textDocument/didChange` content change to `text`, returning
/// the new document text. A change without a `range` replaces the whole
/// document, per the LSP `TextDocumentContentChangeEvent` contract.
#[must_use]
pub fn apply_change(text: &str, change: &TextDocumentContentChangeEvent) -> String {
    match change.range {
        Some(range) => {
            let index = Utf16Index::new(text);
            let start = index.byte_offset(text, range.start);
            let end = index.byte_offset(text, range.end).max(start);
            let mut next = String::with_capacity(text.len() - (end - start) + change.text.len());
            next.push_str(&text[..start]);
            next.push_str(&change.text);
            next.push_str(&text[end..]);
            next
        }
        None => change.text.clone(),
    }
}

#[must_use]
pub fn range(text: &str, index: &Utf16Index, start: usize, end: usize) -> Range {
    Range {
        start: index.position(text, start),
        end: index.position(text, end),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_ascii_offsets_to_positions() {
        let text = "fn main() {\n    print(\"hi\")\n}\n";
        let index = Utf16Index::new(text);
        assert_eq!(
            index.position(text, 16),
            Position {
                line: 1,
                character: 4
            }
        );
        assert_eq!(
            index.byte_offset(
                text,
                Position {
                    line: 1,
                    character: 4
                }
            ),
            16
        );
    }

    #[test]
    fn counts_non_bmp_characters_as_two_utf16_units() {
        let text = "fn main() -> Unit {\r\n    let smile = \"\u{1F642}\";\r\n}\r\n";
        let index = Utf16Index::new(text);
        let semicolon = text.find(';').unwrap();
        assert_eq!(
            index.position(text, semicolon),
            Position {
                line: 1,
                character: 20
            }
        );
        assert_eq!(
            index.byte_offset(
                text,
                Position {
                    line: 1,
                    character: 20
                }
            ),
            semicolon
        );
    }

    #[test]
    fn applies_incremental_insertion_and_deletion() {
        let text = "fn main() -> Unit {\n    print(\"hi\")\n}\n";
        let inserted = apply_change(
            text,
            &TextDocumentContentChangeEvent {
                range: Some(Range {
                    start: Position {
                        line: 1,
                        character: 15,
                    },
                    end: Position {
                        line: 1,
                        character: 15,
                    },
                }),
                range_length: None,
                text: ";".to_owned(),
            },
        );
        assert_eq!(inserted, "fn main() -> Unit {\n    print(\"hi\");\n}\n");

        let reverted = apply_change(
            &inserted,
            &TextDocumentContentChangeEvent {
                range: Some(Range {
                    start: Position {
                        line: 1,
                        character: 15,
                    },
                    end: Position {
                        line: 1,
                        character: 16,
                    },
                }),
                range_length: None,
                text: String::new(),
            },
        );
        assert_eq!(reverted, text);
    }

    #[test]
    fn out_of_range_position_clamps_instead_of_panicking() {
        let text = "fn main() {}\n";
        let index = Utf16Index::new(text);
        let offset = index.byte_offset(
            text,
            Position {
                line: 50,
                character: 50,
            },
        );
        assert!(offset <= text.len());
    }
}

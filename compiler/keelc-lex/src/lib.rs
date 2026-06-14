//! Lexer for Keel Core source files.

use keelc_diag::{registry, Diagnostic};
use keelc_span::{SourceId, Span, Spanned};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StringLiteral {
    pub text: String,
    pub interpolations: Vec<Spanned<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TokenKind {
    Identifier(String),
    Int(String),
    Float(String),
    String(StringLiteral),
    Char(String),
    Keyword(Keyword),
    Newline,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Colon,
    Dot,
    Semicolon,
    At,
    Question,
    Underscore,
    Arrow,
    FatArrow,
    Equal,
    EqualEqual,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Pipe,
    AmpAmp,
    PipePipe,
    Bang,
    Eof,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Keyword {
    Fn,
    Let,
    Mut,
    Struct,
    Enum,
    Match,
    If,
    Else,
    Return,
    Use,
    Module,
    True,
    False,
    Test,
    Assert,
    Catch,
    For,
    In,
    While,
    Break,
    Continue,
    Interface,
    Scope,
    Spawn,
    Arena,
    Extern,
    Impl,
    Async,
    Await,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LexOutput {
    pub tokens: Vec<Token>,
    pub diagnostics: Vec<Diagnostic>,
}

#[must_use]
pub fn lex(source: SourceId, text: &str) -> LexOutput {
    Lexer::new(source, text).lex()
}

struct Lexer<'a> {
    source: SourceId,
    text: &'a str,
    offset: usize,
    tokens: Vec<Token>,
    diagnostics: Vec<Diagnostic>,
}

enum InterpolationResult {
    Interpolation(Spanned<String>),
    /// The interpolation ran into a closing `"`; the caller should emit the
    /// partial string token and stop scanning.
    ClosedByQuote,
    /// The interpolation ran into EOF or a newline; the caller should stop
    /// scanning without emitting a string token.
    Unterminated,
}

impl<'a> Lexer<'a> {
    fn new(source: SourceId, text: &'a str) -> Self {
        Self {
            source,
            text,
            offset: 0,
            tokens: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn lex(mut self) -> LexOutput {
        while let Some(ch) = self.peek_char() {
            match ch {
                ' ' | '\t' | '\r' => {
                    self.advance_char();
                }
                '\n' => {
                    let start = self.offset;
                    self.advance_char();
                    self.push(TokenKind::Newline, start, self.offset);
                }
                '/' if self.peek_next_char() == Some('/') => self.skip_line_comment(),
                '0'..='9' => self.lex_number(),
                '"' => self.lex_string(),
                '\'' => self.lex_char(),
                'a'..='z' | 'A'..='Z' | '_' => self.lex_identifier(),
                '(' => self.single(TokenKind::LeftParen),
                ')' => self.single(TokenKind::RightParen),
                '{' => self.single(TokenKind::LeftBrace),
                '}' => self.single(TokenKind::RightBrace),
                '[' => self.single(TokenKind::LeftBracket),
                ']' => self.single(TokenKind::RightBracket),
                ',' => self.single(TokenKind::Comma),
                ':' => self.single(TokenKind::Colon),
                '.' => self.single(TokenKind::Dot),
                '?' => self.single(TokenKind::Question),
                '*' => self.single(TokenKind::Star),
                '%' => self.single(TokenKind::Percent),
                '&' => self.lex_amp(),
                '|' => self.lex_pipe(),
                ';' => {
                    let start = self.offset;
                    self.single(TokenKind::Semicolon);
                    self.diagnostics.push(Diagnostic::error(
                        registry::K0102,
                        Span::new(self.source, start, self.offset),
                        "Keel uses newline-based statement termination; semicolons are not allowed",
                    ));
                }
                '@' => {
                    let start = self.offset;
                    self.single(TokenKind::At);
                    self.diagnostics.push(Diagnostic::error(
                        registry::K0906,
                        Span::new(self.source, start, self.offset),
                        "attributes are not in Keel Core",
                    ));
                }
                '=' => self.lex_equal(),
                '!' => self.lex_bang(),
                '<' => self.lex_less(),
                '>' => self.lex_greater(),
                '-' => self.lex_minus(),
                '+' => self.single(TokenKind::Plus),
                '/' => self.single(TokenKind::Slash),
                other => {
                    let start = self.offset;
                    self.advance_char();
                    self.diagnostics.push(Diagnostic::error(
                        registry::K0001,
                        Span::new(self.source, start, self.offset),
                        format!("unrecognized character `{other}`"),
                    ));
                }
            }
        }

        let eof = self.offset;
        self.push(TokenKind::Eof, eof, eof);
        LexOutput {
            tokens: self.tokens,
            diagnostics: self.diagnostics,
        }
    }

    fn lex_number(&mut self) {
        let start = self.offset;
        self.take_while(|ch| ch.is_ascii_digit() || ch == '_');
        let mut is_float = false;
        if self.peek_char() == Some('.')
            && self.peek_next_char().is_some_and(|c| c.is_ascii_digit())
        {
            is_float = true;
            self.advance_char();
            self.take_while(|ch| ch.is_ascii_digit() || ch == '_');
        }

        let text = self.slice(start, self.offset).to_owned();
        if is_float {
            self.push(TokenKind::Float(text), start, self.offset);
        } else {
            self.push(TokenKind::Int(text), start, self.offset);
        }
    }

    fn lex_string(&mut self) {
        let start = self.offset;
        self.advance_char(); // consume opening `"`
        let mut content = String::new();
        let mut interpolations = Vec::new();
        let mut terminated = false;

        while let Some(ch) = self.peek_char() {
            match ch {
                '"' => {
                    self.advance_char();
                    self.push_string(
                        std::mem::take(&mut content),
                        std::mem::take(&mut interpolations),
                        start,
                        self.offset,
                    );
                    terminated = true;
                    break;
                }
                '\n' => break,
                '{' => {
                    self.advance_char();
                    if self.peek_char() == Some('{') {
                        self.advance_char();
                        content.push('{');
                    } else {
                        match self.scan_interpolation(start) {
                            InterpolationResult::Interpolation(interpolation) => {
                                interpolations.push(interpolation.clone());
                                content.push('{');
                                content.push_str(&interpolation.value);
                                content.push('}');
                            }
                            InterpolationResult::ClosedByQuote => {
                                self.push_string(
                                    std::mem::take(&mut content),
                                    std::mem::take(&mut interpolations),
                                    start,
                                    self.offset,
                                );
                                terminated = true;
                                break;
                            }
                            InterpolationResult::Unterminated => {
                                terminated = true;
                                break;
                            }
                        }
                    }
                }
                '}' => {
                    self.advance_char();
                    if self.peek_char() == Some('}') {
                        self.advance_char();
                        content.push('}');
                    } else {
                        self.recover_unmatched_close_brace(start);
                        self.push_string(
                            std::mem::take(&mut content),
                            std::mem::take(&mut interpolations),
                            start,
                            self.offset,
                        );
                        terminated = true;
                        break;
                    }
                }
                '\\' => {
                    self.advance_char();
                    if let Some(c) = self.peek_char() {
                        content.push('\\');
                        content.push(c);
                        self.advance_char();
                    }
                }
                _ => {
                    content.push(ch);
                    self.advance_char();
                }
            }
        }

        if !terminated {
            self.diagnostics.push(Diagnostic::error(
                registry::K0002,
                Span::new(self.source, start, self.offset),
                "unterminated string literal",
            ));
        }
    }

    /// Scan the body of a string interpolation after the opening `{` has been
    /// consumed.
    fn scan_interpolation(&mut self, string_start: usize) -> InterpolationResult {
        let interp_start = self.offset;
        let mut depth = 1usize;

        while let Some(ch) = self.peek_char() {
            match ch {
                '\n' => {
                    self.diagnostics.push(Diagnostic::error(
                        registry::K0004,
                        Span::new(self.source, string_start, self.offset),
                        "unterminated string interpolation: `{` opened but never closed",
                    ));
                    return InterpolationResult::Unterminated;
                }
                '"' => {
                    self.advance_char();
                    self.diagnostics.push(Diagnostic::error(
                        registry::K0004,
                        Span::new(self.source, string_start, self.offset),
                        "unterminated string interpolation: `{` opened but never closed",
                    ));
                    return InterpolationResult::ClosedByQuote;
                }
                '{' => {
                    self.advance_char();
                    depth += 1;
                }
                '}' => {
                    self.advance_char();
                    depth -= 1;
                    if depth == 0 {
                        let interp_end = self.offset - 1; // before closing `}`
                        let interpolation = self.slice(interp_start, interp_end).to_owned();
                        return InterpolationResult::Interpolation(Spanned::new(
                            interpolation,
                            Span::new(self.source, interp_start, interp_end),
                        ));
                    }
                }
                _ => {
                    self.advance_char();
                }
            }
        }

        // Reached EOF before finding the matching `}`.
        self.diagnostics.push(Diagnostic::error(
            registry::K0004,
            Span::new(self.source, string_start, self.offset),
            "unterminated string interpolation: `{` opened but never closed",
        ));
        InterpolationResult::Unterminated
    }

    /// Consume the remainder of a string literal after a lone `}` that is not
    /// part of an escape or interpolation. Emits a diagnostic and leaves the
    /// offset at the closing `"` or newline.
    fn recover_unmatched_close_brace(&mut self, string_start: usize) {
        self.diagnostics.push(Diagnostic::error(
            registry::K0004,
            Span::new(self.source, string_start, self.offset),
            "unmatched `}` in string literal; use `}}` for a literal `}`",
        ));
        while let Some(c) = self.peek_char() {
            if c == '"' {
                self.advance_char();
                break;
            }
            if c == '\n' {
                break;
            }
            self.advance_char();
        }
    }

    fn lex_char(&mut self) {
        let start = self.offset;
        self.advance_char();
        let content_start = self.offset;
        while let Some(ch) = self.peek_char() {
            self.advance_char();
            if ch == '\'' {
                let end = self.offset - ch.len_utf8();
                let content = self.slice(content_start, end).to_owned();
                self.push(TokenKind::Char(content), start, self.offset);
                return;
            }
            if ch == '\n' {
                break;
            }
        }

        self.diagnostics.push(Diagnostic::error(
            registry::K0002,
            Span::new(self.source, start, self.offset),
            "unterminated character literal",
        ));
    }

    fn lex_identifier(&mut self) {
        let start = self.offset;
        self.take_while(|ch| ch.is_ascii_alphanumeric() || ch == '_');
        let text = self.slice(start, self.offset);
        let kind = match_keyword(text).map_or_else(
            || {
                if text == "_" {
                    TokenKind::Underscore
                } else {
                    TokenKind::Identifier(text.to_owned())
                }
            },
            TokenKind::Keyword,
        );
        self.push(kind, start, self.offset);
    }

    fn lex_equal(&mut self) {
        let start = self.offset;
        self.advance_char();
        if self.peek_char() == Some('=') {
            self.advance_char();
            self.push(TokenKind::EqualEqual, start, self.offset);
        } else if self.peek_char() == Some('>') {
            self.advance_char();
            self.push(TokenKind::FatArrow, start, self.offset);
        } else {
            self.push(TokenKind::Equal, start, self.offset);
        }
    }

    fn lex_bang(&mut self) {
        let start = self.offset;
        self.advance_char();
        if self.peek_char() == Some('=') {
            self.advance_char();
            self.push(TokenKind::BangEqual, start, self.offset);
        } else {
            self.push(TokenKind::Bang, start, self.offset);
        }
    }

    fn lex_amp(&mut self) {
        let start = self.offset;
        self.advance_char();
        if self.peek_char() == Some('&') {
            self.advance_char();
            self.push(TokenKind::AmpAmp, start, self.offset);
        } else {
            self.diagnostics.push(Diagnostic::error(
                registry::K0001,
                Span::new(self.source, start, self.offset),
                "unrecognized character `&`; did you mean `&&`?",
            ));
        }
    }

    fn lex_pipe(&mut self) {
        let start = self.offset;
        self.advance_char();
        if self.peek_char() == Some('|') {
            self.advance_char();
            self.push(TokenKind::PipePipe, start, self.offset);
        } else {
            self.push(TokenKind::Pipe, start, self.offset);
        }
    }

    fn lex_less(&mut self) {
        let start = self.offset;
        self.advance_char();
        if self.peek_char() == Some('=') {
            self.advance_char();
            self.push(TokenKind::LessEqual, start, self.offset);
        } else {
            self.push(TokenKind::Less, start, self.offset);
        }
    }

    fn lex_greater(&mut self) {
        let start = self.offset;
        self.advance_char();
        if self.peek_char() == Some('=') {
            self.advance_char();
            self.push(TokenKind::GreaterEqual, start, self.offset);
        } else {
            self.push(TokenKind::Greater, start, self.offset);
        }
    }

    fn lex_minus(&mut self) {
        let start = self.offset;
        self.advance_char();
        if self.peek_char() == Some('>') {
            self.advance_char();
            self.push(TokenKind::Arrow, start, self.offset);
        } else {
            self.push(TokenKind::Minus, start, self.offset);
        }
    }

    fn skip_line_comment(&mut self) {
        self.advance_char();
        self.advance_char();
        while let Some(ch) = self.peek_char() {
            if ch == '\n' {
                break;
            }
            self.advance_char();
        }
    }

    fn single(&mut self, kind: TokenKind) {
        let start = self.offset;
        self.advance_char();
        self.push(kind, start, self.offset);
    }

    fn push(&mut self, kind: TokenKind, start: usize, end: usize) {
        self.tokens.push(Token {
            kind,
            span: Span::new(self.source, start, end),
        });
    }

    fn push_string(
        &mut self,
        text: String,
        interpolations: Vec<Spanned<String>>,
        start: usize,
        end: usize,
    ) {
        self.push(
            TokenKind::String(StringLiteral {
                text,
                interpolations,
            }),
            start,
            end,
        );
    }

    fn take_while(&mut self, mut predicate: impl FnMut(char) -> bool) {
        while self.peek_char().is_some_and(&mut predicate) {
            self.advance_char();
        }
    }

    fn peek_char(&self) -> Option<char> {
        self.text.get(self.offset..)?.chars().next()
    }

    fn peek_next_char(&self) -> Option<char> {
        let mut chars = self.text.get(self.offset..)?.chars();
        chars.next()?;
        chars.next()
    }

    fn advance_char(&mut self) -> Option<char> {
        let ch = self.peek_char()?;
        self.offset += ch.len_utf8();
        Some(ch)
    }

    fn slice(&self, start: usize, end: usize) -> &str {
        self.text.get(start..end).unwrap_or("")
    }
}

fn match_keyword(text: &str) -> Option<Keyword> {
    Some(match text {
        "fn" => Keyword::Fn,
        "let" => Keyword::Let,
        "mut" => Keyword::Mut,
        "struct" => Keyword::Struct,
        "enum" => Keyword::Enum,
        "match" => Keyword::Match,
        "if" => Keyword::If,
        "else" => Keyword::Else,
        "return" => Keyword::Return,
        "use" => Keyword::Use,
        "module" => Keyword::Module,
        "true" => Keyword::True,
        "false" => Keyword::False,
        "test" => Keyword::Test,
        "assert" => Keyword::Assert,
        "catch" => Keyword::Catch,
        "for" => Keyword::For,
        "in" => Keyword::In,
        "while" => Keyword::While,
        "break" => Keyword::Break,
        "continue" => Keyword::Continue,
        "interface" => Keyword::Interface,
        "scope" => Keyword::Scope,
        "spawn" => Keyword::Spawn,
        "arena" => Keyword::Arena,
        "extern" => Keyword::Extern,
        "impl" => Keyword::Impl,
        "async" => Keyword::Async,
        "await" => Keyword::Await,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::{lex, Keyword, StringLiteral, TokenKind};
    use keelc_diag::registry;
    use keelc_span::SourceId;

    #[test]
    fn lexes_core_tokens_and_skips_comments() {
        let out = lex(SourceId::new(0), "fn main() {\n// skip\nprint(\"hi\")\n}\n");
        let kinds: Vec<_> = out.tokens.into_iter().map(|t| t.kind).collect();

        assert_eq!(
            kinds,
            vec![
                TokenKind::Keyword(Keyword::Fn),
                TokenKind::Identifier("main".into()),
                TokenKind::LeftParen,
                TokenKind::RightParen,
                TokenKind::LeftBrace,
                TokenKind::Newline,
                TokenKind::Newline,
                TokenKind::Identifier("print".into()),
                TokenKind::LeftParen,
                TokenKind::String(StringLiteral {
                    text: "hi".into(),
                    interpolations: Vec::new(),
                }),
                TokenKind::RightParen,
                TokenKind::Newline,
                TokenKind::RightBrace,
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
        assert!(out.diagnostics.is_empty());
    }

    #[test]
    fn reports_semicolon_with_stable_code() {
        let out = lex(SourceId::new(0), "fn main(){ print(\"hi\"); }\n");

        assert_eq!(out.diagnostics[0].code, registry::K0102);
    }

    #[test]
    fn tracks_only_real_string_interpolations() {
        let out = lex(
            SourceId::new(0),
            "fn main() {\nprint(\"{{1 + 2.0}}\")\nprint(\"{x + y}\")\n}\n",
        );

        let mut strings = out.tokens.iter().filter_map(|token| {
            if let TokenKind::String(literal) = &token.kind {
                Some(literal)
            } else {
                None
            }
        });
        let escaped = strings.next().expect("escaped string token");
        let interpolated = strings.next().expect("interpolated string token");

        assert_eq!(escaped.text, "{1 + 2.0}");
        assert!(escaped.interpolations.is_empty());
        assert_eq!(interpolated.text, "{x + y}");
        assert_eq!(
            interpolated
                .interpolations
                .first()
                .map(|interpolation| interpolation.value.as_str()),
            Some("x + y")
        );
    }
}

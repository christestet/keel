//! Recursive-descent parser for Keel Core source files.

use keelc_ast::{
    BinaryOp, Block, EnumDecl, Expr, FieldDecl, FunctionDecl, Item, MatchArm, Module, Param,
    Pattern, Stmt, StructDecl, StructLiteralField, TestDecl, Type, UnaryOp, UseDecl, VariantDecl,
};
use keelc_diag::{registry, Diagnostic};
use keelc_lex::{lex, Keyword, LexOutput, Token, TokenKind};
use keelc_span::{SourceId, Span, Spanned};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParseOutput {
    pub module: Module,
    pub diagnostics: Vec<Diagnostic>,
}

#[must_use]
pub fn parse(source: SourceId, text: &str) -> ParseOutput {
    let LexOutput {
        tokens,
        diagnostics,
    } = lex(source, text);
    Parser::new(source, tokens, diagnostics).parse_module()
}

struct Parser {
    source: SourceId,
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<Diagnostic>,
    allow_struct_literals: bool,
}

impl Parser {
    fn new(source: SourceId, tokens: Vec<Token>, diagnostics: Vec<Diagnostic>) -> Self {
        Self {
            source,
            tokens,
            pos: 0,
            diagnostics,
            allow_struct_literals: true,
        }
    }

    fn parse_module(mut self) -> ParseOutput {
        self.skip_separators();
        let header = if self.at_keyword(Keyword::Module) {
            Some(self.parse_module_header())
        } else {
            None
        };

        let mut items = Vec::new();
        while !self.at_eof() {
            self.skip_separators();
            if self.at_eof() {
                break;
            }
            if let Some(item) = self.parse_item() {
                items.push(item);
            } else {
                self.advance();
            }
        }

        ParseOutput {
            module: Module { header, items },
            diagnostics: self.diagnostics,
        }
    }

    fn parse_module_header(&mut self) -> Spanned<String> {
        let start = self
            .expect_keyword(Keyword::Module)
            .unwrap_or_else(|| self.empty_span());
        let name = self.expect_identifier("expected module name");
        self.consume_until_separator();
        Spanned::new(name.value, start.join(name.span))
    }

    fn parse_item(&mut self) -> Option<Item> {
        match self.current_kind() {
            Some(TokenKind::Keyword(Keyword::Use)) => Some(Item::Use(self.parse_use())),
            Some(TokenKind::Keyword(Keyword::Struct)) => Some(Item::Struct(self.parse_struct())),
            Some(TokenKind::Keyword(Keyword::Enum)) => Some(Item::Enum(self.parse_enum())),
            Some(TokenKind::Keyword(Keyword::Fn)) => Some(Item::Function(self.parse_function())),
            Some(TokenKind::Keyword(Keyword::Test)) => Some(Item::Test(self.parse_test())),
            Some(TokenKind::Keyword(Keyword::Interface)) => {
                self.banned_keyword(
                    Keyword::Interface,
                    registry::K0902,
                    "interfaces are not in Keel Core",
                );
                None
            }
            Some(TokenKind::Keyword(Keyword::Extern)) => {
                self.banned_keyword(
                    Keyword::Extern,
                    registry::K0905,
                    "extern/FFI is not in Keel Core",
                );
                None
            }
            Some(TokenKind::Keyword(Keyword::Async | Keyword::Await)) => {
                self.diagnostic_current(registry::K0908, "async/await are not in Keel Core");
                self.advance();
                None
            }
            Some(TokenKind::At) => {
                self.diagnostic_current(registry::K0906, "attributes are not in Keel Core");
                self.advance();
                None
            }
            _ => {
                self.diagnostic_current(registry::K0003, "expected top-level declaration");
                None
            }
        }
    }

    fn parse_use(&mut self) -> UseDecl {
        let start = self
            .expect_keyword(Keyword::Use)
            .unwrap_or_else(|| self.empty_span());
        let mut path = Vec::new();
        path.push(self.expect_identifier("expected import path"));
        while self.eat_kind(&TokenKind::Dot).is_some() {
            path.push(self.expect_identifier("expected import path segment"));
        }
        let end = path.last().map_or(start, |segment| segment.span);
        UseDecl {
            path,
            span: start.join(end),
        }
    }

    fn parse_struct(&mut self) -> StructDecl {
        let start = self
            .expect_keyword(Keyword::Struct)
            .unwrap_or_else(|| self.empty_span());
        let name = self.expect_identifier("expected struct name");
        if !is_upper_camel(&name.value) {
            self.diagnostics.push(Diagnostic::error(
                registry::K0101,
                name.span,
                "type names must be UpperCamelCase",
            ));
        }
        let fields = self.parse_braced_fields();
        let end = fields.last().map_or(name.span, |field| field.span);
        StructDecl {
            name,
            fields,
            span: start.join(end),
        }
    }

    fn parse_enum(&mut self) -> EnumDecl {
        let start = self
            .expect_keyword(Keyword::Enum)
            .unwrap_or_else(|| self.empty_span());
        let name = self.expect_identifier("expected enum name");
        if !is_upper_camel(&name.value) {
            self.diagnostics.push(Diagnostic::error(
                registry::K0101,
                name.span,
                "type names must be UpperCamelCase",
            ));
        }

        let mut variants = Vec::new();
        self.expect_kind(&TokenKind::LeftBrace, "expected `{` after enum name");
        self.skip_separators();
        while !self.at_eof() && !self.at_kind(&TokenKind::RightBrace) {
            let variant_start = self.current_span();
            let variant_name = self.expect_identifier("expected enum variant name");
            let fields = if self.eat_kind(&TokenKind::LeftParen).is_some() {
                let payload = self.parse_fields_until(&TokenKind::RightParen);
                self.expect_kind(&TokenKind::RightParen, "expected `)` after enum payload");
                payload
            } else {
                Vec::new()
            };
            let end = fields.last().map_or(variant_name.span, |field| field.span);
            variants.push(VariantDecl {
                name: variant_name,
                fields,
                span: variant_start.join(end),
            });
            self.eat_kind(&TokenKind::Comma);
            self.skip_separators();
        }
        let end = self
            .expect_kind(
                &TokenKind::RightBrace,
                "expected `}` after enum declaration",
            )
            .unwrap_or_else(|| variants.last().map_or(name.span, |variant| variant.span));

        EnumDecl {
            name,
            variants,
            span: start.join(end),
        }
    }

    fn parse_function(&mut self) -> FunctionDecl {
        let start = self
            .expect_keyword(Keyword::Fn)
            .unwrap_or_else(|| self.empty_span());
        let name = if self.current_is_operator() {
            self.diagnostic_current(registry::K0907, "operator overloading is not in Keel Core");
            let token = self.advance().unwrap_or_else(|| missing_token(self.source));
            Spanned::new(self.token_text(&token.kind), token.span)
        } else {
            let name = self.expect_identifier("expected function name");
            if !is_snake_case(&name.value) {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0101,
                    name.span,
                    "value and function names must be snake_case",
                ));
            }
            name
        };

        if self.at_kind(&TokenKind::Less) {
            self.diagnostic_current(
                registry::K0901,
                "user-defined generics are not in Keel Core",
            );
            self.skip_balanced_angle_list();
        }

        let params = self.parse_params();
        let return_type = if self.eat_kind(&TokenKind::Arrow).is_some() {
            Some(self.parse_type())
        } else {
            None
        };
        let body = if self.at_kind(&TokenKind::LeftBrace) {
            Some(self.parse_block())
        } else {
            None
        };
        let end = body.as_ref().map_or_else(
            || return_type.as_ref().map_or(name.span, Type::span),
            |block| block.span,
        );

        FunctionDecl {
            name,
            params,
            return_type,
            body,
            span: start.join(end),
        }
    }

    fn parse_test(&mut self) -> TestDecl {
        let start = self
            .expect_keyword(Keyword::Test)
            .unwrap_or_else(|| self.empty_span());
        let name = match self.advance() {
            Some(Token {
                kind: TokenKind::String(text),
                span,
            }) => Spanned::new(text, span),
            Some(token) => {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0003,
                    token.span,
                    "expected test name string",
                ));
                Spanned::new(String::new(), token.span)
            }
            None => Spanned::new(String::new(), self.empty_span()),
        };
        let body = self.parse_block();
        TestDecl {
            span: start.join(body.span),
            name,
            body,
        }
    }

    fn parse_braced_fields(&mut self) -> Vec<FieldDecl> {
        self.expect_kind(&TokenKind::LeftBrace, "expected `{` before fields");
        let fields = self.parse_fields_until(&TokenKind::RightBrace);
        self.expect_kind(&TokenKind::RightBrace, "expected `}` after fields");
        fields
    }

    fn parse_fields_until(&mut self, end: &TokenKind) -> Vec<FieldDecl> {
        let mut fields = Vec::new();
        self.skip_separators();
        while !self.at_eof() && !self.at_kind(end) {
            let field_start = self.current_span();
            let name = self.expect_identifier("expected field name");
            self.expect_kind(&TokenKind::Colon, "expected `:` after field name");
            let ty = self.parse_type();
            let default = if self.eat_kind(&TokenKind::Equal).is_some() {
                Some(self.parse_expr())
            } else {
                None
            };
            let span = field_start.join(default.as_ref().map_or_else(|| ty.span(), Expr::span));
            fields.push(FieldDecl {
                name,
                ty,
                default,
                span,
            });
            self.eat_kind(&TokenKind::Comma);
            self.skip_separators();
        }
        fields
    }

    fn parse_params(&mut self) -> Vec<Param> {
        let mut params = Vec::new();
        self.expect_kind(&TokenKind::LeftParen, "expected `(` before parameters");
        self.skip_separators();
        while !self.at_eof() && !self.at_kind(&TokenKind::RightParen) {
            let start = self.current_span();
            let name = self.expect_identifier("expected parameter name");
            let ty = if self.eat_kind(&TokenKind::Colon).is_some() {
                Some(self.parse_type())
            } else {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0302,
                    name.span,
                    "function parameters require explicit types",
                ));
                None
            };
            let end = ty.as_ref().map_or(name.span, Type::span);
            params.push(Param {
                name,
                ty,
                span: start.join(end),
            });
            if self.eat_kind(&TokenKind::Comma).is_none() {
                break;
            }
            self.skip_separators();
        }
        self.expect_kind(&TokenKind::RightParen, "expected `)` after parameters");
        params
    }

    fn parse_type(&mut self) -> Type {
        let first = self.parse_named_type();
        let mut members = vec![first];
        while self.eat_kind(&TokenKind::Pipe).is_some() {
            members.push(self.parse_named_type());
        }
        if members.len() == 1 {
            members.remove(0)
        } else {
            let start = members
                .first()
                .map_or_else(|| self.empty_span(), Type::span);
            let end = members.last().map_or(start, Type::span);
            Type::Union {
                members,
                span: start.join(end),
            }
        }
    }

    fn parse_named_type(&mut self) -> Type {
        let name = self.expect_identifier("expected type name");
        let mut args = Vec::new();
        if self.eat_kind(&TokenKind::Less).is_some() {
            self.skip_separators();
            while !self.at_eof() && !self.at_kind(&TokenKind::Greater) {
                args.push(self.parse_type());
                if self.eat_kind(&TokenKind::Comma).is_none() {
                    break;
                }
                self.skip_separators();
            }
            self.expect_kind(&TokenKind::Greater, "expected `>` after type arguments");
        }
        let span = args.last().map_or(name.span, Type::span);
        let full_span = name.span.join(span);
        Type::Named {
            name,
            args,
            span: full_span,
        }
    }

    fn parse_block(&mut self) -> Block {
        let start = self
            .expect_kind(&TokenKind::LeftBrace, "expected `{` before block")
            .unwrap_or_else(|| self.empty_span());
        let mut statements = Vec::new();
        self.skip_separators();
        while !self.at_eof() && !self.at_kind(&TokenKind::RightBrace) {
            statements.push(self.parse_stmt());
            self.skip_separators();
        }
        let end = self
            .expect_kind(&TokenKind::RightBrace, "expected `}` after block")
            .unwrap_or_else(|| statements.last().map_or(start, Stmt::span));
        Block {
            statements,
            span: start.join(end),
        }
    }

    fn parse_stmt(&mut self) -> Stmt {
        match self.current_kind() {
            Some(TokenKind::Keyword(Keyword::Let)) => self.parse_let(false),
            Some(TokenKind::Keyword(Keyword::Mut)) => self.parse_let(true),
            Some(TokenKind::Keyword(Keyword::Return)) => self.parse_return_stmt(),
            Some(TokenKind::Keyword(Keyword::Break)) => {
                let span = self.advance().map_or_else(|| self.empty_span(), |t| t.span);
                Stmt::Break(span)
            }
            Some(TokenKind::Keyword(Keyword::Continue)) => {
                let span = self.advance().map_or_else(|| self.empty_span(), |t| t.span);
                Stmt::Continue(span)
            }
            Some(TokenKind::Keyword(Keyword::Assert)) => self.parse_assert(),
            _ => {
                let expr = self.parse_expr();
                if self.eat_kind(&TokenKind::Equal).is_some() {
                    let value = self.parse_expr();
                    let span = expr.span().join(value.span());
                    Stmt::Assign {
                        target: expr,
                        value,
                        span,
                    }
                } else {
                    Stmt::Expr(expr)
                }
            }
        }
    }

    fn parse_let(&mut self, mutable: bool) -> Stmt {
        let start = self.advance().map_or_else(|| self.empty_span(), |t| t.span);
        let name = self.expect_identifier("expected binding name");
        self.expect_kind(&TokenKind::Equal, "expected `=` in binding");
        let value = self.parse_expr();
        Stmt::Let {
            mutable,
            span: start.join(value.span()),
            name,
            value,
        }
    }

    fn parse_return_stmt(&mut self) -> Stmt {
        let start = self.advance().map_or_else(|| self.empty_span(), |t| t.span);
        let value = if self.at_separator() || self.at_kind(&TokenKind::RightBrace) {
            None
        } else {
            Some(self.parse_expr())
        };
        let span = value.as_ref().map_or(start, |expr| start.join(expr.span()));
        Stmt::Return { value, span }
    }

    fn parse_assert(&mut self) -> Stmt {
        let start = self.advance().map_or_else(|| self.empty_span(), |t| t.span);
        let value = self.parse_expr();
        Stmt::Assert {
            span: start.join(value.span()),
            value,
        }
    }

    fn parse_expr(&mut self) -> Expr {
        self.parse_binary(0)
    }

    fn parse_expr_without_struct_literal(&mut self) -> Expr {
        let old = self.allow_struct_literals;
        self.allow_struct_literals = false;
        let expr = self.parse_expr();
        self.allow_struct_literals = old;
        expr
    }

    fn parse_binary(&mut self, min_prec: u8) -> Expr {
        let mut left = self.parse_postfix();
        let mut prev_was_comparison = false;
        while let Some((op, prec)) = self.current_binary_op() {
            if prec < min_prec {
                break;
            }
            if prev_was_comparison && is_comparison_op(op) {
                self.diagnostic_current(
                    registry::K0003,
                    "comparison operators cannot be chained; use parentheses",
                );
                break;
            }
            prev_was_comparison = is_comparison_op(op);
            self.advance();
            let right = self.parse_binary(prec + 1);
            let span = left.span().join(right.span());
            left = Expr::Binary {
                left: Box::new(left),
                op,
                right: Box::new(right),
                span,
            };
        }
        left
    }

    fn parse_postfix(&mut self) -> Expr {
        let mut expr = self.parse_unary();
        loop {
            if self.eat_kind(&TokenKind::LeftParen).is_some() {
                let mut args = Vec::new();
                self.skip_separators();
                while !self.at_eof() && !self.at_kind(&TokenKind::RightParen) {
                    args.push(self.parse_expr());
                    if self.eat_kind(&TokenKind::Comma).is_none() {
                        break;
                    }
                    self.skip_separators();
                }
                let end = self
                    .expect_kind(&TokenKind::RightParen, "expected `)` after arguments")
                    .unwrap_or_else(|| args.last().map_or_else(|| expr.span(), Expr::span));
                expr = Expr::Call {
                    span: expr.span().join(end),
                    callee: Box::new(expr),
                    args,
                };
            } else if self.eat_kind(&TokenKind::Dot).is_some() {
                let field = self.expect_identifier("expected field name");
                expr = Expr::Field {
                    span: expr.span().join(field.span),
                    target: Box::new(expr),
                    field,
                };
            } else if self.allow_struct_literals && self.at_kind(&TokenKind::LeftBrace) {
                if let Expr::Name(name) = expr {
                    expr = self.finish_struct_literal(name);
                } else {
                    break;
                }
            } else if self.eat_kind(&TokenKind::Question).is_some() {
                let span = expr.span();
                expr = Expr::Question {
                    expr: Box::new(expr),
                    span,
                };
            } else if self.at_keyword(Keyword::Catch) {
                expr = self.finish_catch(expr);
            } else {
                break;
            }
        }
        expr
    }

    fn parse_unary(&mut self) -> Expr {
        if self.at_kind(&TokenKind::Minus) {
            let start = self.advance().map_or_else(|| self.empty_span(), |t| t.span);
            let expr = self.parse_unary();
            Expr::Unary {
                span: start.join(expr.span()),
                op: UnaryOp::Negate,
                expr: Box::new(expr),
            }
        } else if self.at_kind(&TokenKind::Bang) {
            let start = self.advance().map_or_else(|| self.empty_span(), |t| t.span);
            let expr = self.parse_unary();
            Expr::Unary {
                span: start.join(expr.span()),
                op: UnaryOp::Not,
                expr: Box::new(expr),
            }
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Expr {
        match self.advance() {
            Some(Token {
                kind: TokenKind::Int(value),
                span,
            }) => Expr::Int(Spanned::new(value, span)),
            Some(Token {
                kind: TokenKind::Float(value),
                span,
            }) => Expr::Float(Spanned::new(value, span)),
            Some(Token {
                kind: TokenKind::String(value),
                span,
            }) => Expr::String(Spanned::new(value, span)),
            Some(Token {
                kind: TokenKind::Char(value),
                span,
            }) => Expr::Char(Spanned::new(value, span)),
            Some(Token {
                kind: TokenKind::Identifier(value),
                span,
            }) => {
                if value == "nil" || value == "null" {
                    self.diagnostics.push(Diagnostic::error(
                        registry::K0201,
                        span,
                        "Option<T> is the only absence value in Keel Core",
                    ));
                }
                Expr::Name(Spanned::new(value, span))
            }
            Some(Token {
                kind: TokenKind::Underscore,
                span,
            }) => Expr::Wildcard(span),
            Some(Token {
                kind: TokenKind::Keyword(Keyword::True),
                span,
            }) => Expr::Bool(Spanned::new(true, span)),
            Some(Token {
                kind: TokenKind::Keyword(Keyword::False),
                span,
            }) => Expr::Bool(Spanned::new(false, span)),
            Some(Token {
                kind: TokenKind::Keyword(Keyword::If),
                span,
            }) => self.finish_if(span),
            Some(Token {
                kind: TokenKind::Keyword(Keyword::Match),
                span,
            }) => self.finish_match(span),
            Some(Token {
                kind: TokenKind::Keyword(Keyword::While),
                span,
            }) => self.finish_while(span),
            Some(Token {
                kind: TokenKind::Keyword(Keyword::Return),
                span,
            }) => self.finish_return_expr(span),
            Some(Token {
                kind: TokenKind::Keyword(Keyword::Scope | Keyword::Spawn),
                span,
            }) => {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0903,
                    span,
                    "scope/spawn are not in Keel Core",
                ));
                Expr::Missing(span)
            }
            Some(Token {
                kind: TokenKind::Keyword(Keyword::Arena),
                span,
            }) => {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0904,
                    span,
                    "arena is not in Keel Core",
                ));
                Expr::Missing(span)
            }
            Some(Token {
                kind: TokenKind::Keyword(Keyword::Async | Keyword::Await),
                span,
            }) => {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0908,
                    span,
                    "async/await are not in Keel Core",
                ));
                Expr::Missing(span)
            }
            Some(Token {
                kind: TokenKind::LeftParen,
                span: _,
            }) => {
                let expr = self.parse_expr();
                self.expect_kind(&TokenKind::RightParen, "expected `)` after expression");
                expr
            }
            Some(Token {
                kind: TokenKind::LeftBrace,
                span,
            }) => {
                self.pos = self.pos.saturating_sub(1);
                Expr::Block(self.parse_block_from_start(span))
            }
            Some(token) => {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0003,
                    token.span,
                    "expected expression",
                ));
                Expr::Missing(token.span)
            }
            None => Expr::Missing(self.empty_span()),
        }
    }

    fn finish_struct_literal(&mut self, name: Spanned<String>) -> Expr {
        let start = name.span;
        self.expect_kind(&TokenKind::LeftBrace, "expected `{` after struct name");
        let mut fields = Vec::new();
        self.skip_separators();
        while !self.at_eof() && !self.at_kind(&TokenKind::RightBrace) {
            let field_start = self.current_span();
            let field_name = self.expect_identifier("expected struct literal field");
            self.expect_kind(&TokenKind::Colon, "expected `:` after field name");
            let value = self.parse_expr();
            fields.push(StructLiteralField {
                span: field_start.join(value.span()),
                name: field_name,
                value,
            });
            self.eat_kind(&TokenKind::Comma);
            self.skip_separators();
        }
        let end = self
            .expect_kind(&TokenKind::RightBrace, "expected `}` after struct literal")
            .unwrap_or_else(|| fields.last().map_or(start, |field| field.span));
        Expr::StructLiteral {
            name,
            fields,
            span: start.join(end),
        }
    }

    fn finish_if(&mut self, start: Span) -> Expr {
        let condition = self.parse_expr_without_struct_literal();
        let then_block = self.parse_block();
        let else_branch = if self.eat_keyword(Keyword::Else).is_some() {
            Some(Box::new(if self.at_keyword(Keyword::If) {
                let span = self.advance().map_or_else(|| self.empty_span(), |t| t.span);
                self.finish_if(span)
            } else {
                Expr::Block(self.parse_block())
            }))
        } else {
            None
        };
        let end = else_branch
            .as_ref()
            .map_or(then_block.span, |branch| branch.span());
        Expr::If {
            condition: Box::new(condition),
            then_block,
            else_branch,
            span: start.join(end),
        }
    }

    fn finish_match(&mut self, start: Span) -> Expr {
        let scrutinee = self.parse_expr_without_struct_literal();
        let arms = self.parse_match_arms();
        let end = arms.last().map_or_else(|| scrutinee.span(), |arm| arm.span);
        Expr::Match {
            scrutinee: Box::new(scrutinee),
            arms,
            span: start.join(end),
        }
    }

    fn finish_while(&mut self, start: Span) -> Expr {
        let condition = self.parse_expr_without_struct_literal();
        let body = self.parse_block();
        Expr::While {
            condition: Box::new(condition),
            span: start.join(body.span),
            body,
        }
    }

    fn finish_return_expr(&mut self, start: Span) -> Expr {
        let value = if self.at_separator() || self.at_kind(&TokenKind::RightBrace) {
            None
        } else {
            Some(Box::new(self.parse_expr()))
        };
        let span = value.as_ref().map_or(start, |expr| start.join(expr.span()));
        Expr::Return { value, span }
    }

    fn finish_catch(&mut self, expr: Expr) -> Expr {
        let start = expr.span();
        self.expect_keyword(Keyword::Catch);
        let error_name = self.expect_identifier("expected catch error binding");
        let arms = self.parse_match_arms();
        let end = arms.last().map_or(error_name.span, |arm| arm.span);
        Expr::Catch {
            expr: Box::new(expr),
            error_name,
            arms,
            span: start.join(end),
        }
    }

    fn parse_match_arms(&mut self) -> Vec<MatchArm> {
        let mut arms = Vec::new();
        self.expect_kind(&TokenKind::LeftBrace, "expected `{` before match arms");
        self.skip_separators();
        while !self.at_eof() && !self.at_kind(&TokenKind::RightBrace) {
            let pattern = self.parse_pattern();
            let guard = if self.eat_keyword(Keyword::If).is_some() {
                Some(self.parse_expr())
            } else {
                None
            };
            self.expect_kind(&TokenKind::FatArrow, "expected `=>` in match arm");
            let value = self.parse_expr();
            let span = pattern.span().join(value.span());
            arms.push(MatchArm {
                pattern,
                guard,
                value,
                span,
            });
            self.eat_kind(&TokenKind::Comma);
            self.skip_separators();
        }
        self.expect_kind(&TokenKind::RightBrace, "expected `}` after match arms");
        arms
    }

    fn parse_pattern(&mut self) -> Pattern {
        if let Some(span) = self.eat_kind(&TokenKind::Underscore) {
            return Pattern::Wildcard(span);
        }
        let name = self.expect_identifier("expected pattern");
        let mut args = Vec::new();
        if self.eat_kind(&TokenKind::LeftParen).is_some() {
            self.skip_separators();
            while !self.at_eof() && !self.at_kind(&TokenKind::RightParen) {
                args.push(self.parse_pattern());
                if self.eat_kind(&TokenKind::Comma).is_none() {
                    break;
                }
                self.skip_separators();
            }
            self.expect_kind(&TokenKind::RightParen, "expected `)` after pattern");
        }
        let end = args.last().map_or(name.span, Pattern::span);
        let full_span = name.span.join(end);
        Pattern::Name {
            name,
            args,
            span: full_span,
        }
    }

    fn parse_block_from_start(&mut self, _start: Span) -> Block {
        self.parse_block()
    }

    fn current_binary_op(&self) -> Option<(BinaryOp, u8)> {
        let (op, prec) = match self.current_kind()? {
            TokenKind::PipePipe => (BinaryOp::Or, 1),
            TokenKind::AmpAmp => (BinaryOp::And, 2),
            TokenKind::EqualEqual => (BinaryOp::Equal, 3),
            TokenKind::BangEqual => (BinaryOp::NotEqual, 3),
            TokenKind::Less => (BinaryOp::Less, 4),
            TokenKind::LessEqual => (BinaryOp::LessEqual, 4),
            TokenKind::Greater => (BinaryOp::Greater, 4),
            TokenKind::GreaterEqual => (BinaryOp::GreaterEqual, 4),
            TokenKind::Plus => (BinaryOp::Add, 5),
            TokenKind::Minus => (BinaryOp::Subtract, 5),
            TokenKind::Star => (BinaryOp::Multiply, 6),
            TokenKind::Slash => (BinaryOp::Divide, 6),
            TokenKind::Percent => (BinaryOp::Remainder, 6),
            _ => return None,
        };
        Some((op, prec))
    }

    fn skip_balanced_angle_list(&mut self) {
        let mut depth = 0usize;
        while let Some(token) = self.advance() {
            match token.kind {
                TokenKind::Less => depth += 1,
                TokenKind::Greater => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                TokenKind::Eof | TokenKind::Newline | TokenKind::LeftParen => break,
                _ => {}
            }
        }
    }

    fn banned_keyword(&mut self, keyword: Keyword, code: keelc_diag::Code, message: &str) {
        if let Some(span) = self.expect_keyword(keyword) {
            self.diagnostics
                .push(Diagnostic::error(code, span, message));
        }
        if self.at_kind(&TokenKind::LeftBrace) {
            self.skip_braced_tokens();
        } else {
            self.consume_until_separator();
        }
    }

    fn skip_braced_tokens(&mut self) {
        let mut depth = 0usize;
        while let Some(token) = self.advance() {
            match token.kind {
                TokenKind::LeftBrace => depth += 1,
                TokenKind::RightBrace => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        break;
                    }
                }
                TokenKind::Eof => break,
                _ => {}
            }
        }
    }

    fn consume_until_separator(&mut self) {
        while !self.at_eof() && !self.at_separator() {
            self.advance();
        }
    }

    fn skip_separators(&mut self) {
        while self.at_separator() {
            self.advance();
        }
    }

    fn at_separator(&self) -> bool {
        self.at_kind(&TokenKind::Newline) || self.at_kind(&TokenKind::Semicolon)
    }

    fn expect_identifier(&mut self, message: &str) -> Spanned<String> {
        match self.advance() {
            Some(Token {
                kind: TokenKind::Identifier(value),
                span,
            }) => Spanned::new(value, span),
            Some(Token {
                kind: TokenKind::Keyword(keyword),
                span,
            }) => Spanned::new(keyword_text(keyword).to_owned(), span),
            Some(token) => {
                self.diagnostics
                    .push(Diagnostic::error(registry::K0003, token.span, message));
                Spanned::new(String::new(), token.span)
            }
            None => Spanned::new(String::new(), self.empty_span()),
        }
    }

    fn expect_keyword(&mut self, keyword: Keyword) -> Option<Span> {
        if self.at_keyword(keyword) {
            return self.advance().map(|token| token.span);
        }
        self.diagnostic_current(registry::K0003, "expected keyword");
        None
    }

    fn eat_keyword(&mut self, keyword: Keyword) -> Option<Span> {
        if self.at_keyword(keyword) {
            self.advance().map(|token| token.span)
        } else {
            None
        }
    }

    fn expect_kind(&mut self, kind: &TokenKind, message: &str) -> Option<Span> {
        if self.at_kind(kind) {
            return self.advance().map(|token| token.span);
        }
        self.diagnostic_current(registry::K0003, message);
        None
    }

    fn eat_kind(&mut self, kind: &TokenKind) -> Option<Span> {
        if self.at_kind(kind) {
            self.advance().map(|token| token.span)
        } else {
            None
        }
    }

    fn at_keyword(&self, keyword: Keyword) -> bool {
        matches!(self.current_kind(), Some(TokenKind::Keyword(found)) if *found == keyword)
    }

    fn at_kind(&self, kind: &TokenKind) -> bool {
        self.current_kind()
            .is_some_and(|current| same_token_kind(current, kind))
    }

    fn at_eof(&self) -> bool {
        self.at_kind(&TokenKind::Eof) || self.pos >= self.tokens.len()
    }

    fn current_is_operator(&self) -> bool {
        matches!(
            self.current_kind(),
            Some(
                TokenKind::Plus
                    | TokenKind::Minus
                    | TokenKind::Star
                    | TokenKind::Slash
                    | TokenKind::Percent
                    | TokenKind::EqualEqual
                    | TokenKind::BangEqual
                    | TokenKind::Less
                    | TokenKind::LessEqual
                    | TokenKind::Greater
                    | TokenKind::GreaterEqual
                    | TokenKind::AmpAmp
                    | TokenKind::PipePipe
                    | TokenKind::Bang
            )
        )
    }

    fn current_kind(&self) -> Option<&TokenKind> {
        self.tokens.get(self.pos).map(|token| &token.kind)
    }

    fn current_span(&self) -> Span {
        self.tokens
            .get(self.pos)
            .map_or_else(|| self.empty_span(), |token| token.span)
    }

    fn advance(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.pos).cloned();
        if token.is_some() {
            self.pos += 1;
        }
        token
    }

    fn diagnostic_current(&mut self, code: keelc_diag::Code, message: &str) {
        self.diagnostics
            .push(Diagnostic::error(code, self.current_span(), message));
    }

    fn empty_span(&self) -> Span {
        Span::empty(self.source, 0)
    }

    fn token_text(&self, kind: &TokenKind) -> String {
        match kind {
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::EqualEqual => "==",
            TokenKind::BangEqual => "!=",
            TokenKind::Less => "<",
            TokenKind::LessEqual => "<=",
            TokenKind::Greater => ">",
            TokenKind::GreaterEqual => ">=",
            TokenKind::AmpAmp => "&&",
            TokenKind::PipePipe => "||",
            TokenKind::Bang => "!",
            _ => "",
        }
        .to_owned()
    }
}

fn is_comparison_op(op: BinaryOp) -> bool {
    matches!(
        op,
        BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual
    )
}

fn same_token_kind(left: &TokenKind, right: &TokenKind) -> bool {
    std::mem::discriminant(left) == std::mem::discriminant(right)
}

fn missing_token(source: SourceId) -> Token {
    Token {
        kind: TokenKind::Eof,
        span: Span::empty(source, 0),
    }
}

fn keyword_text(keyword: Keyword) -> &'static str {
    match keyword {
        Keyword::Fn => "fn",
        Keyword::Let => "let",
        Keyword::Mut => "mut",
        Keyword::Struct => "struct",
        Keyword::Enum => "enum",
        Keyword::Match => "match",
        Keyword::If => "if",
        Keyword::Else => "else",
        Keyword::Return => "return",
        Keyword::Use => "use",
        Keyword::Module => "module",
        Keyword::True => "true",
        Keyword::False => "false",
        Keyword::Test => "test",
        Keyword::Assert => "assert",
        Keyword::Catch => "catch",
        Keyword::For => "for",
        Keyword::In => "in",
        Keyword::While => "while",
        Keyword::Break => "break",
        Keyword::Continue => "continue",
        Keyword::Interface => "interface",
        Keyword::Scope => "scope",
        Keyword::Spawn => "spawn",
        Keyword::Arena => "arena",
        Keyword::Extern => "extern",
        Keyword::Impl => "impl",
        Keyword::Async => "async",
        Keyword::Await => "await",
    }
}

fn is_upper_camel(name: &str) -> bool {
    let mut chars = name.chars();
    chars.next().is_some_and(char::is_uppercase) && !name.contains('_')
}

fn is_snake_case(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|ch| ch.is_ascii_lowercase() || ch == '_')
        && name
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
}

#[cfg(test)]
mod tests {
    use super::parse;
    use keelc_diag::registry;
    use keelc_span::SourceId;

    #[test]
    fn parses_function_and_call() {
        let out = parse(SourceId::new(0), "fn main() {\n    print(\"hello\")\n}\n");

        assert!(out.diagnostics.is_empty());
        assert_eq!(out.module.items.len(), 1);
    }

    #[test]
    fn parses_match_scrutinee_before_arm_block() {
        let out = parse(
            SourceId::new(0),
            "enum Status { Active, }\nfn main() {\n    match s {\n        Active => print(\"active\"),\n    }\n}\n",
        );

        assert!(out.diagnostics.is_empty());
    }

    #[test]
    fn reports_banned_user_generics() {
        let out = parse(SourceId::new(0), "fn identity<T>(x: T) -> T {\n    x\n}\n");

        assert!(out
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == registry::K0901));
    }

    #[test]
    fn reports_missing_parameter_type() {
        let out = parse(
            SourceId::new(0),
            "fn add(a, b: Int) -> Int {\n    a + b\n}\n",
        );

        assert!(out
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == registry::K0302));
    }
}

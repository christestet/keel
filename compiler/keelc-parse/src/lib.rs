//! Recursive-descent parser for Keel Core source files.

use keelc_ast::{
    BinaryOp, Block, EnumDecl, Expr, FieldDecl, FunctionDecl, ImplDecl, InterfaceDecl, Item,
    MatchArm, Module, Param, Pattern, Stmt, StringLiteral as AstStringLiteral, StructDecl,
    StructLiteralField, TestDecl, Type, TypeParam, UnaryOp, UseDecl, VariantDecl,
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
    parse_with_milestone(source, text, 1)
}

#[must_use]
pub fn parse_with_milestone(source: SourceId, text: &str, milestone: u32) -> ParseOutput {
    let LexOutput {
        tokens,
        diagnostics,
    } = lex(source, text);
    Parser::new(source, tokens, diagnostics, milestone).parse_module()
}

/// Parse a single expression that appears inside a string interpolation.
///
/// Interpolations are stored as raw text in the lexer/parser, so semantic
/// stages and backends re-parse them as expressions. The helper wraps the
/// snippet in a dummy function and returns the first expression statement from
/// that function's body, or `None` if the snippet is empty or malformed.
#[must_use]
pub fn parse_interpolation_expr(source: SourceId, expr_text: &str) -> Option<Expr> {
    let wrapped = format!("fn __keel_interp() {{\n{expr_text}\n}}\n");
    let output = parse(source, &wrapped);
    if !output.diagnostics.is_empty() {
        return None;
    }

    output.module.items.iter().find_map(|item| {
        let Item::Function(function) = item else {
            return None;
        };
        let body = function.body.as_ref()?;
        body.statements.iter().find_map(|statement| {
            if let Stmt::Expr(expr) = statement {
                Some(expr.clone())
            } else {
                None
            }
        })
    })
}

struct Parser {
    source: SourceId,
    tokens: Vec<Token>,
    pos: usize,
    diagnostics: Vec<Diagnostic>,
    allow_struct_literals: bool,
    milestone: u32,
}

impl Parser {
    fn new(
        source: SourceId,
        tokens: Vec<Token>,
        diagnostics: Vec<Diagnostic>,
        milestone: u32,
    ) -> Self {
        Self {
            source,
            tokens,
            pos: 0,
            diagnostics,
            allow_struct_literals: true,
            milestone,
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
                if self.milestone >= 5 {
                    Some(Item::Interface(self.parse_interface()))
                } else {
                    self.banned_keyword(
                        Keyword::Interface,
                        registry::K0902,
                        "interfaces are not in Keel Core",
                    );
                    None
                }
            }
            Some(TokenKind::Keyword(Keyword::Impl)) => {
                if self.milestone >= 5 {
                    Some(Item::Impl(self.parse_impl()))
                } else {
                    self.banned_keyword(
                        Keyword::Impl,
                        registry::K0003,
                        "expected top-level declaration",
                    );
                    None
                }
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
        let type_params = if self.milestone >= 5 && self.at_kind(&TokenKind::LeftBracket) {
            self.parse_type_params()
        } else {
            Vec::new()
        };
        let fields = self.parse_braced_fields();
        let end = fields.last().map_or(name.span, |field| field.span);
        StructDecl {
            name,
            type_params,
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

        let type_params = if self.milestone >= 5 && self.at_kind(&TokenKind::LeftBracket) {
            self.parse_type_params()
        } else {
            Vec::new()
        };
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
            type_params,
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

        let type_params = if self.milestone >= 5 && self.at_kind(&TokenKind::LeftBracket) {
            self.parse_type_params()
        } else if self.at_kind(&TokenKind::Less) {
            self.diagnostic_current(
                registry::K0901,
                "user-defined generics are not in Keel Core",
            );
            self.skip_balanced_angle_list();
            Vec::new()
        } else {
            Vec::new()
        };

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
            type_params,
            params,
            return_type,
            body,
            span: start.join(end),
        }
    }

    fn parse_interface(&mut self) -> InterfaceDecl {
        let start = self
            .expect_keyword(Keyword::Interface)
            .unwrap_or_else(|| self.empty_span());
        let name = self.expect_identifier("expected interface name");
        if !is_upper_camel(&name.value) {
            self.diagnostics.push(Diagnostic::error(
                registry::K0101,
                name.span,
                "interface names must be UpperCamelCase",
            ));
        }
        self.expect_kind(&TokenKind::LeftBrace, "expected `{` after interface name");
        self.skip_separators();
        let mut methods = Vec::new();
        while !self.at_eof() && !self.at_kind(&TokenKind::RightBrace) {
            methods.push(self.parse_function());
            self.skip_separators();
        }
        let end = self
            .expect_kind(
                &TokenKind::RightBrace,
                "expected `}` after interface declaration",
            )
            .unwrap_or_else(|| methods.last().map_or(name.span, |method| method.span));
        InterfaceDecl {
            name,
            methods,
            span: start.join(end),
        }
    }

    fn parse_impl(&mut self) -> ImplDecl {
        let start = self
            .expect_keyword(Keyword::Impl)
            .unwrap_or_else(|| self.empty_span());
        let interface_name = self.expect_identifier("expected interface name");
        if !is_upper_camel(&interface_name.value) {
            self.diagnostics.push(Diagnostic::error(
                registry::K0101,
                interface_name.span,
                "interface names must be UpperCamelCase",
            ));
        }
        self.expect_keyword(Keyword::For);
        let type_name = self.expect_identifier("expected type name");
        if !is_upper_camel(&type_name.value) {
            self.diagnostics.push(Diagnostic::error(
                registry::K0101,
                type_name.span,
                "type names must be UpperCamelCase",
            ));
        }
        let type_args = if self.milestone >= 5 && self.at_kind(&TokenKind::LeftBracket) {
            self.parse_type_args_in_brackets()
        } else {
            Vec::new()
        };
        self.expect_kind(&TokenKind::LeftBrace, "expected `{` after impl header");
        self.skip_separators();
        let mut methods = Vec::new();
        while !self.at_eof() && !self.at_kind(&TokenKind::RightBrace) {
            methods.push(self.parse_function());
            self.skip_separators();
        }
        let end = self
            .expect_kind(&TokenKind::RightBrace, "expected `}` after impl block")
            .unwrap_or_else(|| methods.last().map_or(type_name.span, |method| method.span));
        ImplDecl {
            interface_name,
            type_name,
            type_args,
            methods,
            span: start.join(end),
        }
    }

    fn parse_test(&mut self) -> TestDecl {
        let start = self
            .expect_keyword(Keyword::Test)
            .unwrap_or_else(|| self.empty_span());
        let name = match self.advance() {
            Some(Token {
                kind: TokenKind::String(literal),
                span,
            }) => Spanned::new(literal.text, span),
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
            let is_self = name.value == "self";
            let ty = if self.eat_kind(&TokenKind::Colon).is_some() {
                Some(self.parse_type())
            } else if is_self {
                None
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

    fn parse_type_params(&mut self) -> Vec<TypeParam> {
        let mut params = Vec::new();
        let bracket_span = self
            .expect_kind(
                &TokenKind::LeftBracket,
                "expected `[` before type parameters",
            )
            .unwrap_or_else(|| self.empty_span());
        self.skip_separators();
        let mut seen_names: Vec<String> = Vec::new();
        while !self.at_eof() && !self.at_kind(&TokenKind::RightBracket) {
            let start = self.current_span();
            let name = self.expect_identifier("expected type parameter name");
            let bound = if self.eat_kind(&TokenKind::Colon).is_some() {
                let bound_name = self.expect_identifier("expected interface bound name");
                if !is_upper_camel(&bound_name.value) {
                    self.diagnostics.push(Diagnostic::error(
                        registry::K0101,
                        bound_name.span,
                        "interface names must be UpperCamelCase",
                    ));
                }
                Some(bound_name)
            } else {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0801,
                    name.span,
                    format!(
                        "type parameter `{}` must have an interface bound",
                        name.value
                    ),
                ));
                None
            };
            let span = start.join(bound.as_ref().map_or(name.span, |b| name.span.join(b.span)));
            if seen_names.contains(&name.value) {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0804,
                    name.span,
                    format!("duplicate type parameter name `{}`", name.value),
                ));
            }
            seen_names.push(name.value.clone());
            if matches!(
                name.value.as_str(),
                "Int" | "Float" | "Bool" | "String" | "Char" | "Unit"
            ) {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0805,
                    name.span,
                    format!(
                        "type parameter `{}` shadows built-in type `{}`",
                        name.value, name.value
                    ),
                ));
            }
            params.push(TypeParam { name, bound, span });
            if self.eat_kind(&TokenKind::Comma).is_none() {
                break;
            }
            self.skip_separators();
        }
        if params.len() > 256 {
            self.diagnostics.push(Diagnostic::error(
                registry::K0806,
                bracket_span,
                "too many type parameters; at most 256 are allowed",
            ));
        }
        self.expect_kind(
            &TokenKind::RightBracket,
            "expected `]` after type parameters",
        );
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
        let ty = if self.eat_kind(&TokenKind::Colon).is_some() {
            Some(self.parse_type())
        } else {
            None
        };
        self.expect_kind(&TokenKind::Equal, "expected `=` in binding");
        let value = self.parse_expr();
        let end = value.span();
        Stmt::Let {
            mutable,
            span: start.join(end),
            name,
            ty,
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
                    type_args: Vec::new(),
                    args,
                };
            } else if self.milestone >= 5
                && self.at_kind(&TokenKind::LeftBracket)
                && matches!(expr, Expr::Name(_))
            {
                let type_args = self.parse_type_args_in_brackets();
                // After type args, check for call `(args)` or struct literal `{...}`
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
                        type_args,
                        args,
                    };
                } else if self.allow_struct_literals && self.at_kind(&TokenKind::LeftBrace) {
                    if let Expr::Name(name) = expr {
                        expr = self.finish_struct_literal(name, type_args);
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else if self.eat_kind(&TokenKind::Dot).is_some() {
                let field = self.expect_identifier("expected field name");
                if self.at_kind(&TokenKind::LeftParen) {
                    let mut args = Vec::new();
                    self.advance();
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
                    expr = Expr::MethodCall {
                        span: expr.span().join(end),
                        receiver: Box::new(expr),
                        method: field,
                        args,
                    };
                } else {
                    expr = Expr::Field {
                        span: expr.span().join(field.span),
                        target: Box::new(expr),
                        field,
                    };
                }
            } else if self.allow_struct_literals && self.at_kind(&TokenKind::LeftBrace) {
                if let Expr::Name(name) = expr {
                    expr = self.finish_struct_literal(name, Vec::new());
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
            }) => Expr::String(Spanned::new(
                AstStringLiteral {
                    text: value.text,
                    interpolations: value.interpolations,
                },
                span,
            )),
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
            }) => self.banned_expr(span, registry::K0903, "scope/spawn are not in Keel Core"),
            Some(Token {
                kind: TokenKind::Keyword(Keyword::Arena),
                span,
            }) => self.banned_expr(span, registry::K0904, "arena is not in Keel Core"),
            Some(Token {
                kind: TokenKind::Keyword(Keyword::Async | Keyword::Await),
                span,
            }) => self.banned_expr(span, registry::K0908, "async/await are not in Keel Core"),
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

    fn finish_struct_literal(&mut self, name: Spanned<String>, type_args: Vec<Type>) -> Expr {
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
            type_args,
            fields,
            span: start.join(end),
        }
    }

    fn parse_type_args_in_brackets(&mut self) -> Vec<Type> {
        self.expect_kind(
            &TokenKind::LeftBracket,
            "expected `[` before type arguments",
        );
        let mut args = Vec::new();
        self.skip_separators();
        while !self.at_eof() && !self.at_kind(&TokenKind::RightBracket) {
            args.push(self.parse_type());
            if self.eat_kind(&TokenKind::Comma).is_none() {
                break;
            }
            self.skip_separators();
        }
        self.expect_kind(
            &TokenKind::RightBracket,
            "expected `]` after type arguments",
        );
        args
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

    fn banned_expr(&mut self, span: Span, code: keelc_diag::Code, message: &str) -> Expr {
        self.diagnostics
            .push(Diagnostic::error(code, span, message));
        Expr::Missing(span)
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
    use keelc_ast::pretty::pretty_print;
    use keelc_diag::{registry, Severity};
    use keelc_span::SourceId;

    fn no_errors(diagnostics: &[keelc_diag::Diagnostic]) -> bool {
        diagnostics.iter().all(|d| d.severity != Severity::Error)
    }

    fn format_idempotent(source: &str) {
        let first = parse(SourceId::new(0), source);
        assert!(
            no_errors(&first.diagnostics),
            "parse errors: {:?}",
            first.diagnostics
        );
        let once = pretty_print(&first.module);
        let reparsed = parse(SourceId::new(1), &once);
        assert!(
            no_errors(&reparsed.diagnostics),
            "formatter produced invalid source:\n{once}\nerrors: {:?}",
            reparsed.diagnostics
        );
        let twice = pretty_print(&reparsed.module);
        assert_eq!(
            once, twice,
            "formatter is not idempotent:\n--- once ---\n{once}\n--- twice ---\n{twice}"
        );
    }

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

    #[test]
    fn formatter_hello_world() {
        format_idempotent("fn main() {\n    print(\"hello\")\n}\n");
    }

    #[test]
    fn formatter_struct_and_enum() {
        format_idempotent(
            "struct Engine {\n    power: Int\n}\n\nenum Event {\n    Login(user: User),\n    Logout,\n}\n",
        );
    }

    #[test]
    fn formatter_if_and_match() {
        format_idempotent(
            "enum S { A, B }\nfn main() {\n    if true {\n        1\n    } else {\n        2\n    }\n    match S.A {\n        A => 1\n        B => 2\n    }\n}\n",
        );
    }

    #[test]
    fn formatter_string_interpolation_and_escapes() {
        format_idempotent(
            "fn main() {\n    print(\"{x}\")\n    print(\"{{not interpolated}}\")\n}\n",
        );
    }

    #[test]
    fn formatter_nested_expressions() {
        format_idempotent("fn main() {\n    let x = 1 + 2 * 3\n    let y = (1 + 2) * 3\n    let z = a && b || c\n}\n");
    }

    #[test]
    fn formatter_catch_and_question() {
        format_idempotent(
            "enum E { Bad, Other }\nfn main() -> Result<Int, E> {\n    let x = f() catch err {\n        Bad => return Err(Bad),\n        other => return Err(other),\n    }\n    let y = g()?\n    Ok(y)\n}\n",
        );
    }

    #[test]
    fn formatter_idempotent_on_conformance() {
        let manifest = std::env!("CARGO_MANIFEST_DIR");
        let suite = std::path::Path::new(manifest).join("../../tests/conformance");
        let mut count = 0;
        for entry in std::fs::read_dir(&suite).expect("read conformance suite") {
            let entry = entry.expect("read dir entry");
            let dir = entry.path();
            if !dir.is_dir() {
                continue;
            }
            let source_path = dir.join("main.keel");
            let stdout_path = dir.join("expected.stdout");
            if !source_path.is_file() || !stdout_path.is_file() {
                continue;
            }
            let source = std::fs::read_to_string(&source_path).expect("read case");
            let first = parse(SourceId::new(count), &source);
            if first
                .diagnostics
                .iter()
                .any(|d| d.severity == Severity::Error)
            {
                continue;
            }
            let once = pretty_print(&first.module);
            let reparsed = parse(SourceId::new(count + 1_000), &once);
            assert!(
                !reparsed
                    .diagnostics
                    .iter()
                    .any(|d| d.severity == Severity::Error),
                "{}: formatter produced invalid source:\n{once}",
                dir.display()
            );
            let twice = pretty_print(&reparsed.module);
            assert_eq!(
                once,
                twice,
                "{}: formatter is not idempotent",
                dir.display()
            );
            count += 1;
        }
        assert!(
            count >= 60,
            "expected at least 60 accept cases, found {count}"
        );
    }
}

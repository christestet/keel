//! Lower the typed AST to KIR.
//!
//! The lowerer is responsible for:
//!
//! * stripping AST spans and producing a compact, explicitly-typed KIR tree;
//! * resolving the type of every expression via [`TypeContext`];
//! * desugaring `?` and `catch` into explicit match/return sequences;
//! * parsing string interpolations so backends see complete expressions.

use crate::{
    BinaryOp, Block, EnumDecl, Expr, Field, FunctionDecl, ImplDecl, InterfaceDecl, Item, MatchArm,
    Method, Module, Param, Pattern, Route, RouteHandler, Stmt, StringLiteral, StringPart,
    StructDecl, TestDecl, Variant,
};
use keelc_ast::{Item as AstItem, StringLiteral as AstStringLiteral};
use keelc_diag::{registry, Diagnostic};
use keelc_span::{LineIndex, Span, Spanned};
use keelc_types::infer::{question_success_type, TypeContext};
use keelc_types::{reduce_error_types, TypeInfo};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LowerOutput {
    pub module: Module,
    pub diagnostics: Vec<Diagnostic>,
}

/// Lower an AST module to KIR.  The source text is required to compute source
/// line numbers for `assert` statements.
#[must_use]
pub fn lower(module: &keelc_ast::Module, source: &str) -> LowerOutput {
    Lowerer::new(module, source).lower()
}

struct Lowerer<'a> {
    module: &'a keelc_ast::Module,
    line_index: LineIndex,
    ctx: TypeContext,
    diagnostics: Vec<Diagnostic>,
    temp_index: usize,
}

impl<'a> Lowerer<'a> {
    fn new(module: &'a keelc_ast::Module, source: &'a str) -> Self {
        Self {
            module,
            line_index: LineIndex::new(source),
            ctx: TypeContext::new(module),
            diagnostics: Vec::new(),
            temp_index: 0,
        }
    }

    fn lower(mut self) -> LowerOutput {
        let name = self
            .module
            .header
            .as_ref()
            .map(|header| header.value.clone());
        let mut items = Vec::new();
        for item in &self.module.items {
            if let Some(kir_item) = self.lower_item(item) {
                items.push(kir_item);
            }
        }
        LowerOutput {
            module: Module { name, items },
            diagnostics: self.diagnostics,
        }
    }

    fn lower_item(&mut self, item: &AstItem) -> Option<Item> {
        match item {
            AstItem::Struct(decl) => Some(Item::Struct(self.lower_struct_decl(decl))),
            AstItem::Enum(decl) => Some(Item::Enum(self.lower_enum_decl(decl))),
            AstItem::Function(decl) => self.lower_function(decl).map(Item::Function),
            AstItem::Interface(decl) => Some(Item::Interface(self.lower_interface_decl(decl))),
            AstItem::Impl(decl) => Some(Item::Impl(self.lower_impl_decl(decl))),
            AstItem::Test(decl) => Some(Item::Test(self.lower_test_decl(decl))),
            AstItem::Use(_) => None,
        }
    }

    fn lower_struct_decl(&mut self, decl: &keelc_ast::StructDecl) -> StructDecl {
        let type_params = keelc_types::type_param_bounds(&decl.type_params);
        StructDecl {
            name: decl.name.value.clone(),
            fields: decl
                .fields
                .iter()
                .map(|field| Field {
                    name: field.name.value.clone(),
                    ty: keelc_types::substitute_type_params(
                        &TypeInfo::from_ast(&field.ty),
                        &type_params,
                    ),
                    default: field.default.as_ref().map(|expr| self.lower_expr(expr)),
                })
                .collect(),
        }
    }

    fn lower_enum_decl(&mut self, decl: &keelc_ast::EnumDecl) -> EnumDecl {
        EnumDecl {
            name: decl.name.value.clone(),
            variants: decl
                .variants
                .iter()
                .map(|variant| Variant {
                    name: variant.name.value.clone(),
                    fields: variant
                        .fields
                        .iter()
                        .map(|field| Field {
                            name: field.name.value.clone(),
                            ty: TypeInfo::from_ast(&field.ty),
                            default: None,
                        })
                        .collect(),
                })
                .collect(),
        }
    }

    fn lower_function(&mut self, decl: &keelc_ast::FunctionDecl) -> Option<FunctionDecl> {
        let Some(body) = &decl.body else {
            self.diagnostics.push(Diagnostic::error(
                registry::K0003,
                decl.span,
                "function body required for code generation",
            ));
            return None;
        };

        let type_params = keelc_types::type_param_bounds(&decl.type_params);
        let return_type = keelc_types::substitute_type_params(
            &decl
                .return_type
                .as_ref()
                .map_or(TypeInfo::Unit, TypeInfo::from_ast),
            &type_params,
        );

        let mut params = Vec::new();
        self.ctx.push_scope();
        for param in &decl.params {
            let ty = keelc_types::substitute_type_params(
                &param
                    .ty
                    .as_ref()
                    .map_or(TypeInfo::Unknown, TypeInfo::from_ast),
                &type_params,
            );
            if param.ty.is_none() && param.name.value != "self" {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0302,
                    param.name.span,
                    "function parameters require explicit types",
                ));
            }
            params.push(Param {
                name: param.name.value.clone(),
                ty: ty.clone(),
            });
            self.ctx.define_value(&param.name.value, ty);
        }

        let previous_return = self.ctx.current_return_type().cloned();
        self.ctx.set_current_return_type(return_type.clone());
        let body = self.lower_block(body);
        self.ctx.clear_current_return_type();
        if let Some(previous) = previous_return {
            self.ctx.set_current_return_type(previous);
        }
        self.ctx.pop_scope();

        Some(FunctionDecl {
            name: decl.name.value.clone(),
            params,
            return_type,
            body,
        })
    }

    fn lower_interface_decl(&mut self, decl: &keelc_ast::InterfaceDecl) -> InterfaceDecl {
        InterfaceDecl {
            name: decl.name.value.clone(),
            methods: decl
                .methods
                .iter()
                .map(|method| Method {
                    name: method.name.value.clone(),
                    params: method
                        .params
                        .iter()
                        .filter(|param| param.name.value != "self")
                        .map(|param| Param {
                            name: param.name.value.clone(),
                            ty: param
                                .ty
                                .as_ref()
                                .map_or(TypeInfo::Unknown, TypeInfo::from_ast),
                        })
                        .collect(),
                    return_type: method
                        .return_type
                        .as_ref()
                        .map_or(TypeInfo::Unit, TypeInfo::from_ast),
                })
                .collect(),
        }
    }

    fn lower_impl_decl(&mut self, decl: &keelc_ast::ImplDecl) -> ImplDecl {
        ImplDecl {
            interface_name: decl.interface_name.value.clone(),
            type_name: decl.type_name.value.clone(),
            methods: decl
                .methods
                .iter()
                .filter_map(|method| {
                    self.lower_function(method).map(|mut lowered| {
                        lowered.params.retain(|param| param.name != "self");
                        lowered
                    })
                })
                .collect(),
        }
    }

    fn lower_test_decl(&mut self, decl: &keelc_ast::TestDecl) -> TestDecl {
        self.ctx.push_scope();
        let previous_return = self.ctx.current_return_type().cloned();
        self.ctx.set_current_return_type(TypeInfo::Unit);
        let body = self.lower_block(&decl.body);
        self.ctx.clear_current_return_type();
        if let Some(previous) = previous_return {
            self.ctx.set_current_return_type(previous);
        }
        self.ctx.pop_scope();
        TestDecl {
            name: decl.name.value.clone(),
            body,
        }
    }

    fn lower_block(&mut self, block: &keelc_ast::Block) -> Block {
        self.ctx.push_scope();
        let mut statements = Vec::new();
        for statement in &block.statements {
            statements.extend(self.lower_stmt(statement));
        }
        let ty = statements
            .last()
            .map_or(TypeInfo::Unit, |statement| match statement {
                Stmt::Expr(expr) => expr_ty(expr),
                Stmt::Return { value: Some(expr) } => expr_ty(expr),
                _ => TypeInfo::Unit,
            });
        self.ctx.pop_scope();
        Block { statements, ty }
    }

    fn lower_stmt(&mut self, statement: &keelc_ast::Stmt) -> Vec<Stmt> {
        match statement {
            keelc_ast::Stmt::Let {
                name, ty, value, ..
            } => {
                let value_ty = ty
                    .as_ref()
                    .map(TypeInfo::from_ast)
                    .unwrap_or_else(|| self.ctx.infer_expr(value));

                match value {
                    keelc_ast::Expr::Question { expr, .. } => {
                        let (mut stmts, result) = self.desugar_question_expr(expr);
                        self.ctx.define_value(&name.value, value_ty.clone());
                        stmts.push(Stmt::Let {
                            name: name.value.clone(),
                            ty: value_ty,
                            value: result,
                        });
                        stmts
                    }
                    keelc_ast::Expr::Catch {
                        expr,
                        error_name,
                        arms,
                        ..
                    } => self.desugar_catch_stmt(&name.value, expr, error_name, arms),
                    _ => {
                        let value = self.lower_expr(value);
                        self.ctx.define_value(&name.value, value_ty.clone());
                        vec![Stmt::Let {
                            name: name.value.clone(),
                            ty: value_ty,
                            value,
                        }]
                    }
                }
            }
            keelc_ast::Stmt::Assign { target, value, .. } => {
                let target = self.lower_expr(target);
                let value = self.lower_expr(value);
                vec![Stmt::Assign { target, value }]
            }
            keelc_ast::Stmt::Return { value, .. } => {
                let value = value.as_ref().map(|expr| self.lower_expr(expr));
                vec![Stmt::Return { value }]
            }
            keelc_ast::Stmt::Break(_) => vec![Stmt::Break],
            keelc_ast::Stmt::Continue(_) => vec![Stmt::Continue],
            keelc_ast::Stmt::Assert { value, span } => {
                let value = self.lower_expr(value);
                let line = self.line_index.line_col(span.start).line;
                vec![Stmt::Assert { value, line }]
            }
            keelc_ast::Stmt::Expr(expr) => {
                let expr = self.lower_expr(expr);
                vec![Stmt::Expr(expr)]
            }
        }
    }

    fn lower_expr(&mut self, expr: &keelc_ast::Expr) -> Expr {
        let ty = self.ctx.infer_expr(expr);
        match expr {
            keelc_ast::Expr::Int(value) => Expr::Int(value.value.replace('_', "")),
            keelc_ast::Expr::Float(value) => Expr::Float(value.value.replace('_', "")),
            keelc_ast::Expr::String(value) => {
                Expr::String(self.lower_string_literal(&value.value, value.span))
            }
            keelc_ast::Expr::Char(value) => Expr::Char(value.value.chars().next().unwrap_or('\0')),
            keelc_ast::Expr::Bool(value) => Expr::Bool(value.value),
            keelc_ast::Expr::Unit(_) => Expr::Unit,
            keelc_ast::Expr::Name(name) => Expr::Name(name.value.clone()),
            keelc_ast::Expr::Wildcard(_) | keelc_ast::Expr::Missing(_) => {
                Expr::Name("__keel_missing".to_string())
            }
            keelc_ast::Expr::Unary { op, expr, .. } => Expr::Unary {
                op: (*op).into(),
                expr: Box::new(self.lower_expr(expr)),
                ty,
            },
            keelc_ast::Expr::Binary {
                left, op, right, ..
            } => Expr::Binary {
                op: (*op).into(),
                left: Box::new(self.lower_expr(left)),
                right: Box::new(self.lower_expr(right)),
                ty,
            },
            keelc_ast::Expr::Call {
                callee,
                type_args,
                args,
                ..
            } => Expr::Call {
                callee: Box::new(self.lower_expr(callee)),
                type_args: type_args
                    .iter()
                    .map(TypeInfo::from_ast)
                    .map(|ty| self.ctx.resolve_type(&ty))
                    .collect(),
                args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                ty,
            },
            keelc_ast::Expr::Field { target, field, .. } => Expr::Field {
                target: Box::new(self.lower_expr(target)),
                field: field.value.clone(),
                ty,
            },
            keelc_ast::Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => Expr::MethodCall {
                receiver: Box::new(self.lower_expr(receiver)),
                method: method.value.clone(),
                args: args.iter().map(|arg| self.lower_expr(arg)).collect(),
                ty,
            },
            keelc_ast::Expr::StructLiteral { name, fields, .. } => Expr::StructLiteral {
                name: name.value.clone(),
                fields: fields
                    .iter()
                    .map(|field| (field.name.value.clone(), self.lower_expr(&field.value)))
                    .collect(),
                ty,
            },
            keelc_ast::Expr::If {
                condition,
                then_block,
                else_branch,
                ..
            } => {
                let else_block = match else_branch {
                    Some(else_branch) => match else_branch.as_ref() {
                        keelc_ast::Expr::Block(block) => self.lower_block(block),
                        _ => {
                            let value = self.lower_expr(else_branch);
                            let value_ty = expr_ty(&value);
                            Block {
                                statements: vec![Stmt::Expr(value)],
                                ty: value_ty,
                            }
                        }
                    },
                    None => Block {
                        statements: Vec::new(),
                        ty: TypeInfo::Unit,
                    },
                };
                Expr::If {
                    condition: Box::new(self.lower_expr(condition)),
                    then_block: self.lower_block(then_block),
                    else_block,
                    ty,
                }
            }
            keelc_ast::Expr::Match {
                scrutinee, arms, ..
            } => {
                let scrutinee_expr = self.lower_expr(scrutinee);
                let scrutinee_ty = self.ctx.infer_expr(scrutinee);
                let arms = arms
                    .iter()
                    .map(|arm| self.lower_match_arm(arm, &scrutinee_ty))
                    .collect();
                Expr::Match {
                    scrutinee: Box::new(scrutinee_expr),
                    arms,
                    ty,
                }
            }
            keelc_ast::Expr::While {
                condition, body, ..
            } => Expr::While {
                condition: Box::new(self.lower_expr(condition)),
                body: self.lower_block(body),
            },
            keelc_ast::Expr::Scope { deadline, body, .. } => {
                let deadline = deadline
                    .as_ref()
                    .map(|deadline| Box::new(self.lower_expr(deadline)));
                let body = self.lower_block(body);
                let error_ty = scope_error_type(&body, deadline.is_some());
                let scope_ty = error_ty.as_ref().map_or_else(
                    || body.ty.clone(),
                    |error| TypeInfo::generic("Result", vec![body.ty.clone(), error.clone()]),
                );
                Expr::Scope {
                    deadline,
                    body,
                    ty: scope_ty,
                    error_ty,
                }
            }
            keelc_ast::Expr::Spawn { expr, .. } => Expr::Spawn {
                expr: Box::new(self.lower_expr(expr)),
                ty,
            },
            keelc_ast::Expr::Block(block) => Expr::Block(self.lower_block(block)),
            keelc_ast::Expr::Question { expr, .. } => {
                let (mut stmts, result) = self.desugar_question_expr(expr);
                let result_ty = expr_ty(&result);
                stmts.push(Stmt::Expr(result));
                Expr::Block(Block {
                    statements: stmts,
                    ty: result_ty,
                })
            }
            keelc_ast::Expr::Catch {
                expr,
                error_name,
                arms,
                ..
            } => {
                let (mut stmts, result, result_ty) =
                    self.desugar_catch_expr(expr, error_name, arms);
                stmts.push(Stmt::Expr(result));
                Expr::Block(Block {
                    statements: stmts,
                    ty: result_ty,
                })
            }
            keelc_ast::Expr::Return { value, .. } => Expr::Return {
                value: value.as_ref().map(|expr| Box::new(self.lower_expr(expr))),
            },
            keelc_ast::Expr::Router { routes, .. } => {
                let routes = routes
                    .iter()
                    .map(|route| {
                        let handler = match &route.handler {
                            keelc_ast::RouteHandler::Closure { param, body, .. } => {
                                self.ctx.push_scope();
                                self.ctx.define_value(
                                    &param.value,
                                    TypeInfo::Named("http.Request".to_string()),
                                );
                                let body = Box::new(self.lower_expr(body));
                                self.ctx.pop_scope();
                                RouteHandler::Closure {
                                    param: param.value.clone(),
                                    body,
                                }
                            }
                            keelc_ast::RouteHandler::Expr(expr) => match expr.as_ref() {
                                keelc_ast::Expr::Name(name) => {
                                    RouteHandler::Named(name.value.clone())
                                }
                                _ => RouteHandler::Named(String::new()),
                            },
                        };
                        Route {
                            pattern: route.pattern.value.clone(),
                            handler,
                        }
                    })
                    .collect();
                Expr::Router { routes, ty }
            }
        }
    }

    fn lower_match_arm(&mut self, arm: &keelc_ast::MatchArm, scrutinee_ty: &TypeInfo) -> MatchArm {
        self.ctx.push_scope();
        let pattern = self.lower_pattern(&arm.pattern, scrutinee_ty);
        self.define_pattern_bindings(&pattern, scrutinee_ty);
        let guard = arm.guard.as_ref().map(|guard| self.lower_expr(guard));
        let value = self.lower_expr(&arm.value);
        self.ctx.pop_scope();
        MatchArm {
            pattern,
            guard,
            value,
        }
    }

    fn lower_pattern(&mut self, pattern: &keelc_ast::Pattern, scrutinee_ty: &TypeInfo) -> Pattern {
        match pattern {
            keelc_ast::Pattern::Wildcard(_) => Pattern::Wildcard,
            keelc_ast::Pattern::Name { name, args, .. } => {
                let payload_types = self.ctx.pattern_payload_types(scrutinee_ty, &name.value);
                Pattern::Name {
                    name: name.value.clone(),
                    args: args
                        .iter()
                        .enumerate()
                        .map(|(index, arg)| {
                            let arg_ty = payload_types
                                .get(index)
                                .cloned()
                                .unwrap_or(TypeInfo::Unknown);
                            self.lower_pattern(arg, &arg_ty)
                        })
                        .collect(),
                    payload_types,
                }
            }
        }
    }

    fn define_pattern_bindings(&mut self, pattern: &Pattern, _scrutinee_ty: &TypeInfo) {
        if let Pattern::Name {
            args,
            payload_types,
            ..
        } = pattern
        {
            for (index, arg) in args.iter().enumerate() {
                let ty = payload_types
                    .get(index)
                    .cloned()
                    .unwrap_or(TypeInfo::Unknown);
                if let Pattern::Name { name, .. } = arg {
                    self.ctx.define_value(name, ty);
                }
            }
        }
    }

    fn lower_string_literal(&mut self, literal: &AstStringLiteral, span: Span) -> StringLiteral {
        let mut parts = Vec::new();
        let mut cursor = 0usize;
        for interpolation in &literal.interpolations {
            let needle = format!("{{{}}}", interpolation.value);
            let Some(relative_start) = literal.text[cursor..].find(&needle) else {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0004,
                    span,
                    "malformed string interpolation",
                ));
                continue;
            };
            let start = cursor + relative_start;
            if start > cursor {
                parts.push(StringPart::Text(literal.text[cursor..start].to_string()));
            }
            match keelc_parse::parse_interpolation_expr(
                interpolation.span.source,
                &interpolation.value,
            ) {
                Some(expr) => {
                    let ty = self.ctx.infer_expr(&expr);
                    parts.push(StringPart::Expr {
                        expr: Box::new(self.lower_expr(&expr)),
                        ty,
                    });
                }
                None => {
                    self.diagnostics.push(Diagnostic::error(
                        registry::K0004,
                        span,
                        "malformed string interpolation",
                    ));
                    parts.push(StringPart::Expr {
                        expr: Box::new(Expr::Name("__keel_bad_interp".to_string())),
                        ty: TypeInfo::Unknown,
                    });
                }
            }
            cursor = start + needle.len();
        }
        if cursor < literal.text.len() {
            parts.push(StringPart::Text(literal.text[cursor..].to_string()));
        }
        if parts.is_empty() {
            parts.push(StringPart::Text(String::new()));
        }
        StringLiteral { parts }
    }

    fn desugar_question_expr(&mut self, expr: &keelc_ast::Expr) -> (Vec<Stmt>, Expr) {
        let expr_type = self.ctx.infer_expr(expr);
        let success_type = question_success_type(&expr_type).unwrap_or(TypeInfo::Unknown);
        let temp = self.next_temp();
        let mut stmts = vec![Stmt::Let {
            name: temp.clone(),
            ty: expr_type.clone(),
            value: self.lower_expr(expr),
        }];

        if let Some((_, error_type)) = expr_type.result_parts() {
            stmts.push(Stmt::Expr(Expr::If {
                condition: Box::new(Expr::Binary {
                    op: BinaryOp::Equal,
                    left: Box::new(Expr::Field {
                        target: Box::new(Expr::Name(temp.clone())),
                        field: "tag".to_string(),
                        ty: TypeInfo::String,
                    }),
                    right: Box::new(string_expr("Err")),
                    ty: TypeInfo::Bool,
                }),
                then_block: Block {
                    statements: vec![Stmt::Return {
                        value: Some(Expr::Call {
                            callee: Box::new(Expr::Name("Err".to_string())),
                            type_args: Vec::new(),
                            args: vec![Expr::Payload {
                                value: Box::new(Expr::Name(temp.clone())),
                                index: 0,
                                ty: error_type.clone(),
                            }],
                            ty: error_type.clone(),
                        }),
                    }],
                    ty: TypeInfo::Unit,
                },
                else_block: Block {
                    statements: Vec::new(),
                    ty: TypeInfo::Unit,
                },
                ty: TypeInfo::Unit,
            }));
        } else if expr_type.option_inner().is_some() {
            stmts.push(Stmt::Expr(Expr::If {
                condition: Box::new(Expr::Binary {
                    op: BinaryOp::Equal,
                    left: Box::new(Expr::Field {
                        target: Box::new(Expr::Name(temp.clone())),
                        field: "tag".to_string(),
                        ty: TypeInfo::String,
                    }),
                    right: Box::new(string_expr("None")),
                    ty: TypeInfo::Bool,
                }),
                then_block: Block {
                    statements: vec![Stmt::Return {
                        value: Some(Expr::Name("None".to_string())),
                    }],
                    ty: TypeInfo::Unit,
                },
                else_block: Block {
                    statements: Vec::new(),
                    ty: TypeInfo::Unit,
                },
                ty: TypeInfo::Unit,
            }));
        } else {
            self.diagnostics.push(Diagnostic::error(
                registry::K0501,
                expr.span(),
                "`?` can only be used on Option or Result values",
            ));
        }

        let result = Expr::Payload {
            value: Box::new(Expr::Name(temp)),
            index: 0,
            ty: success_type.clone(),
        };
        (stmts, result)
    }

    fn desugar_catch_stmt(
        &mut self,
        name: &str,
        expr: &keelc_ast::Expr,
        error_name: &Spanned<String>,
        arms: &[keelc_ast::MatchArm],
    ) -> Vec<Stmt> {
        let (mut stmts, result_expr, result_ty) = self.desugar_catch_expr(expr, error_name, arms);
        self.ctx.define_value(name, result_ty.clone());
        stmts.push(Stmt::Let {
            name: name.to_string(),
            ty: result_ty,
            value: result_expr,
        });
        stmts
    }

    fn desugar_catch_expr(
        &mut self,
        expr: &keelc_ast::Expr,
        error_name: &Spanned<String>,
        arms: &[keelc_ast::MatchArm],
    ) -> (Vec<Stmt>, Expr, TypeInfo) {
        let expr_type = self.ctx.infer_expr(expr);
        let (success_type, error_type) = expr_type
            .result_parts()
            .map(|(ok, err)| (ok.clone(), err.clone()))
            .unwrap_or((TypeInfo::Unknown, TypeInfo::Unknown));
        let result_name = self.next_temp();
        let temp = self.next_temp();
        let mut stmts = Vec::new();

        stmts.push(Stmt::Let {
            name: temp.clone(),
            ty: expr_type.clone(),
            value: self.lower_expr(expr),
        });

        stmts.push(Stmt::Var {
            name: result_name.clone(),
            ty: success_type.clone(),
        });

        let ok_arm = MatchArm {
            pattern: Pattern::Name {
                name: "Ok".to_string(),
                args: vec![Pattern::Name {
                    name: "__keel_ok_value".to_string(),
                    args: Vec::new(),
                    payload_types: Vec::new(),
                }],
                payload_types: vec![success_type.clone()],
            },
            guard: None,
            value: Expr::Block(Block {
                statements: vec![Stmt::Assign {
                    target: Expr::Name(result_name.clone()),
                    value: Expr::Name("__keel_ok_value".to_string()),
                }],
                ty: TypeInfo::Unit,
            }),
        };

        self.ctx.push_scope();
        self.ctx.define_value(&error_name.value, error_type.clone());
        let catch_arms: Vec<_> = arms
            .iter()
            .map(|arm| {
                self.ctx.push_scope();
                let (pattern, binding) =
                    self.lower_catch_pattern(&arm.pattern, &error_type, &error_name.value);
                if let Some((name, ty)) = &binding {
                    self.ctx.define_value(name, ty.clone());
                }
                self.define_pattern_bindings(&pattern, &error_type);
                let guard = arm.guard.as_ref().map(|guard| self.lower_expr(guard));
                let mut value = match &arm.value {
                    keelc_ast::Expr::Return { value, .. } => Expr::Return {
                        value: value.as_ref().map(|v| Box::new(self.lower_expr(v))),
                    },
                    _ => Expr::Block(Block {
                        statements: vec![Stmt::Assign {
                            target: Expr::Name(result_name.clone()),
                            value: self.lower_expr(&arm.value),
                        }],
                        ty: TypeInfo::Unit,
                    }),
                };
                if let Some((name, ty)) = binding {
                    let binding_name = name.clone();
                    value = Expr::Block(Block {
                        statements: vec![
                            Stmt::Let {
                                name,
                                ty,
                                value: Expr::Name(error_name.value.clone()),
                            },
                            Stmt::Assign {
                                target: Expr::Name("_".to_string()),
                                value: Expr::Name(binding_name),
                            },
                            Stmt::Expr(value),
                        ],
                        ty: TypeInfo::Unit,
                    });
                }
                self.ctx.pop_scope();
                MatchArm {
                    pattern,
                    guard,
                    value,
                }
            })
            .collect();
        self.ctx.pop_scope();

        let err_arm = MatchArm {
            pattern: Pattern::Name {
                name: "Err".to_string(),
                args: vec![Pattern::Name {
                    name: error_name.value.clone(),
                    args: Vec::new(),
                    payload_types: Vec::new(),
                }],
                payload_types: vec![error_type.clone()],
            },
            guard: None,
            value: Expr::Block(Block {
                statements: vec![Stmt::Expr(Expr::Match {
                    scrutinee: Box::new(Expr::Name(error_name.value.clone())),
                    arms: catch_arms,
                    ty: TypeInfo::Unit,
                })],
                ty: TypeInfo::Unit,
            }),
        };

        stmts.push(Stmt::Expr(Expr::Match {
            scrutinee: Box::new(Expr::Name(temp)),
            arms: vec![ok_arm, err_arm],
            ty: TypeInfo::Unit,
        }));

        let result = Expr::Name(result_name.clone());
        (stmts, result, success_type)
    }

    fn lower_catch_pattern(
        &mut self,
        pattern: &keelc_ast::Pattern,
        error_type: &TypeInfo,
        _error_name: &str,
    ) -> (Pattern, Option<(String, TypeInfo)>) {
        if let keelc_ast::Pattern::Name { name, args, .. } = pattern {
            if args.is_empty() && self.ctx.enum_variant_type(&name.value).is_none() {
                let ty = if name.value == "other" {
                    error_type.clone()
                } else {
                    TypeInfo::Unknown
                };
                return (Pattern::Wildcard, Some((name.value.clone(), ty)));
            }
        }
        (self.lower_pattern(pattern, error_type), None)
    }

    fn next_temp(&mut self) -> String {
        let temp = format!("__keel_tmp_{}", self.temp_index);
        self.temp_index += 1;
        temp
    }
}

fn string_expr(text: &str) -> Expr {
    Expr::String(StringLiteral {
        parts: vec![StringPart::Text(text.to_string())],
    })
}

fn expr_ty(expr: &Expr) -> TypeInfo {
    match expr {
        Expr::Int(_) => TypeInfo::Int,
        Expr::Float(_) => TypeInfo::Float,
        Expr::String(_) => TypeInfo::String,
        Expr::Char(_) => TypeInfo::Char,
        Expr::Bool(_) => TypeInfo::Bool,
        Expr::Unit => TypeInfo::Unit,
        Expr::Name(_) => TypeInfo::Unknown,
        Expr::Unary { ty, .. }
        | Expr::Binary { ty, .. }
        | Expr::Call { ty, .. }
        | Expr::Field { ty, .. }
        | Expr::MethodCall { ty, .. }
        | Expr::StructLiteral { ty, .. }
        | Expr::If { ty, .. }
        | Expr::Match { ty, .. }
        | Expr::Payload { ty, .. } => ty.clone(),
        Expr::While { .. } | Expr::Return { .. } => TypeInfo::Unit,
        Expr::Spawn { ty, .. } | Expr::Scope { ty, .. } | Expr::Router { ty, .. } => ty.clone(),
        Expr::Block(block) => block.ty.clone(),
    }
}

fn scope_error_type(block: &Block, has_deadline: bool) -> Option<TypeInfo> {
    let mut errors = Vec::new();
    collect_scope_errors(block, &mut errors);
    if has_deadline {
        let cancelled = TypeInfo::Named("Cancelled".to_string());
        if !errors.contains(&cancelled) {
            errors.push(cancelled);
        }
    }
    reduce_error_types(errors)
}

fn collect_scope_errors(block: &Block, errors: &mut Vec<TypeInfo>) {
    for statement in &block.statements {
        match statement {
            Stmt::Let { value, .. } | Stmt::Assign { value, .. } | Stmt::Expr(value) => {
                collect_expr_scope_errors(value, errors);
            }
            Stmt::Return {
                value: Some(value), ..
            }
            | Stmt::Assert { value, .. } => collect_expr_scope_errors(value, errors),
            Stmt::Var { .. } | Stmt::Return { value: None } | Stmt::Break | Stmt::Continue => {}
        }
    }
}

fn collect_expr_scope_errors(expr: &Expr, errors: &mut Vec<TypeInfo>) {
    match expr {
        Expr::Spawn { ty, .. } => {
            if let Some((_, error)) = task_inner(ty).and_then(TypeInfo::result_parts) {
                if !errors.iter().any(|seen| seen == error) {
                    errors.push(error.clone());
                }
            }
        }
        Expr::Unary { expr, .. } => collect_expr_scope_errors(expr, errors),
        Expr::Binary { left, right, .. } => {
            collect_expr_scope_errors(left, errors);
            collect_expr_scope_errors(right, errors);
        }
        Expr::Call { callee, args, .. } => {
            collect_expr_scope_errors(callee, errors);
            for arg in args {
                collect_expr_scope_errors(arg, errors);
            }
        }
        Expr::Field { target, .. } => collect_expr_scope_errors(target, errors),
        Expr::MethodCall { receiver, args, .. } => {
            collect_expr_scope_errors(receiver, errors);
            for arg in args {
                collect_expr_scope_errors(arg, errors);
            }
        }
        Expr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                collect_expr_scope_errors(value, errors);
            }
        }
        Expr::If {
            condition,
            then_block,
            else_block,
            ..
        } => {
            collect_expr_scope_errors(condition, errors);
            collect_scope_errors(then_block, errors);
            collect_scope_errors(else_block, errors);
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            collect_expr_scope_errors(scrutinee, errors);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_expr_scope_errors(guard, errors);
                }
                collect_expr_scope_errors(&arm.value, errors);
            }
        }
        Expr::While { condition, body } => {
            collect_expr_scope_errors(condition, errors);
            collect_scope_errors(body, errors);
        }
        Expr::Scope { .. } => {}
        Expr::Payload { value, .. } => collect_expr_scope_errors(value, errors),
        Expr::Block(block) => collect_scope_errors(block, errors),
        Expr::Router { routes, .. } => {
            for route in routes {
                if let RouteHandler::Closure { body, .. } = &route.handler {
                    collect_expr_scope_errors(body, errors);
                }
            }
        }
        Expr::Return {
            value: Some(value), ..
        } => collect_expr_scope_errors(value, errors),
        Expr::Int(_)
        | Expr::Float(_)
        | Expr::String(_)
        | Expr::Char(_)
        | Expr::Bool(_)
        | Expr::Unit
        | Expr::Name(_)
        | Expr::Return { value: None } => {}
    }
}

fn task_inner(ty: &TypeInfo) -> Option<&TypeInfo> {
    match ty {
        TypeInfo::Generic { name, args } if name == "Task" && args.len() == 1 => args.first(),
        _ => None,
    }
}

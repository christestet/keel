//! Name resolution and early semantic diagnostics for Keel Core.

use keelc_ast::{
    BinaryOp, Block, Expr, Item, MatchArm, Module, Pattern, RouteHandler, Stmt, StringLiteral,
};
use keelc_diag::{registry, Diagnostic};
use keelc_span::{Span, Spanned};
use keelc_types::infer::{is_int_float_pair, type_absorbs, types_compatible, TypeContext};
use keelc_types::{merge_types, reduce_error_types, TypeInfo};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolveOutput {
    pub diagnostics: Vec<Diagnostic>,
}

#[must_use]
pub fn resolve(module: &Module) -> ResolveOutput {
    Resolver::new(module).resolve()
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypecheckOutput {
    pub diagnostics: Vec<Diagnostic>,
}

#[must_use]
pub fn typecheck(module: &Module) -> TypecheckOutput {
    Typechecker::new(module).check()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BindingKind {
    Immutable,
    Mutable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Binding {
    name: String,
    kind: BindingKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StructInfo {
    name: String,
    fields: Vec<StructFieldInfo>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StructFieldInfo {
    name: String,
    has_default: bool,
}

struct Resolver<'a> {
    module: &'a Module,
    structs: Vec<StructInfo>,
    scopes: Vec<Vec<Binding>>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Resolver<'a> {
    fn new(module: &'a Module) -> Self {
        Self {
            module,
            structs: collect_structs(module),
            scopes: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn resolve(mut self) -> ResolveOutput {
        for item in &self.module.items {
            match item {
                Item::Function(function) => {
                    if let Some(body) = &function.body {
                        self.push_scope();
                        for param in &function.params {
                            self.define(&param.name, BindingKind::Immutable);
                        }
                        self.resolve_block(body);
                        self.pop_scope();
                    }
                }
                Item::Test(test) => {
                    self.push_scope();
                    self.resolve_block(&test.body);
                    self.pop_scope();
                }
                Item::Struct(_)
                | Item::Enum(_)
                | Item::Use(_)
                | Item::Interface(_)
                | Item::Impl(_) => {}
            }
        }

        ResolveOutput {
            diagnostics: self.diagnostics,
        }
    }

    fn resolve_block(&mut self, block: &Block) {
        self.push_scope();
        for statement in &block.statements {
            self.resolve_stmt(statement);
        }
        self.pop_scope();
    }

    fn resolve_stmt(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Let {
                mutable,
                name,
                value,
                ..
            } => {
                self.resolve_expr(value);
                let kind = if *mutable {
                    BindingKind::Mutable
                } else {
                    BindingKind::Immutable
                };
                self.define(name, kind);
            }
            Stmt::Assign { target, value, .. } => {
                self.check_assignment_target(target);
                self.resolve_expr(target);
                self.resolve_expr(value);
            }
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.resolve_expr(value);
                }
            }
            Stmt::Assert { value, .. } | Stmt::Expr(value) => self.resolve_expr(value),
            Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn resolve_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Missing(_)
            | Expr::Int(_)
            | Expr::Float(_)
            | Expr::String(_)
            | Expr::Char(_)
            | Expr::Bool(_)
            | Expr::Name(_)
            | Expr::Wildcard(_) => {}
            Expr::Unary { expr, .. } | Expr::Question { expr, .. } => self.resolve_expr(expr),
            Expr::Binary { left, right, .. } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }
            Expr::Call { callee, args, .. } => {
                self.resolve_expr(callee);
                for arg in args {
                    self.resolve_expr(arg);
                }
            }
            Expr::Field { target, .. } => self.resolve_expr(target),
            Expr::MethodCall { receiver, args, .. } => {
                self.resolve_expr(receiver);
                for arg in args {
                    self.resolve_expr(arg);
                }
            }
            Expr::StructLiteral { name, fields, .. } => {
                self.check_struct_literal(name, fields);
                for field in fields {
                    self.resolve_expr(&field.value);
                }
            }
            Expr::If {
                condition,
                then_block,
                else_branch,
                ..
            } => {
                self.resolve_expr(condition);
                self.resolve_block(then_block);
                if let Some(else_branch) = else_branch {
                    self.resolve_expr(else_branch);
                }
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                self.resolve_expr(scrutinee);
                for arm in arms {
                    self.push_scope();
                    self.resolve_expr(&arm.value);
                    self.pop_scope();
                }
            }
            Expr::While {
                condition, body, ..
            } => {
                self.resolve_expr(condition);
                self.resolve_block(body);
            }
            Expr::Scope { deadline, body, .. } => {
                if let Some(deadline) = deadline {
                    self.resolve_expr(deadline);
                }
                self.resolve_block(body);
            }
            Expr::Spawn { expr, .. } => self.resolve_expr(expr),
            Expr::Block(block) => self.resolve_block(block),
            Expr::Catch {
                expr,
                error_name,
                arms,
                ..
            } => {
                self.resolve_expr(expr);
                self.push_scope();
                self.define(error_name, BindingKind::Immutable);
                for arm in arms {
                    self.push_scope();
                    self.resolve_expr(&arm.value);
                    self.pop_scope();
                }
                self.pop_scope();
            }
            Expr::Return { value, .. } => {
                if let Some(value) = value {
                    self.resolve_expr(value);
                }
            }
            Expr::Router { routes, .. } => {
                for route in routes {
                    match &route.handler {
                        RouteHandler::Closure { param, body, .. } => {
                            self.push_scope();
                            self.define(param, BindingKind::Immutable);
                            self.resolve_expr(body);
                            self.pop_scope();
                        }
                        RouteHandler::Expr(expr) => self.resolve_expr(expr),
                    }
                }
            }
        }
    }

    fn check_assignment_target(&mut self, target: &Expr) {
        if let Expr::Name(name) = target {
            if self.binding_kind(&name.value) == Some(BindingKind::Immutable) {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0303,
                    name.span,
                    format!("cannot assign to immutable binding `{}`", name.value),
                ));
            }
        }
    }

    fn check_struct_literal(
        &mut self,
        name: &Spanned<String>,
        fields: &[keelc_ast::StructLiteralField],
    ) {
        let Some(info) = self.structs.iter().find(|info| info.name == name.value) else {
            return;
        };

        let missing = info.fields.iter().find(|field| {
            !field.has_default
                && !fields
                    .iter()
                    .any(|provided| provided.name.value == field.name)
        });

        if let Some(field) = missing {
            self.diagnostics.push(Diagnostic::error(
                registry::K0301,
                name.span,
                format!(
                    "struct `{}` is missing required field `{}`",
                    name.value, field.name
                ),
            ));
        }
    }

    fn define(&mut self, name: &Spanned<String>, kind: BindingKind) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(Binding {
                name: name.value.clone(),
                kind,
            });
        }
    }

    fn binding_kind(&self, name: &str) -> Option<BindingKind> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev())
            .find(|binding| binding.name == name)
            .map(|binding| binding.kind)
    }

    fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    fn pop_scope(&mut self) {
        let _ = self.scopes.pop();
    }
}

fn collect_structs(module: &Module) -> Vec<StructInfo> {
    let mut structs = Vec::new();
    for item in &module.items {
        if let Item::Struct(decl) = item {
            let fields = decl
                .fields
                .iter()
                .map(|field| StructFieldInfo {
                    name: field.name.value.clone(),
                    has_default: field.default.is_some(),
                })
                .collect();
            structs.push(StructInfo {
                name: decl.name.value.clone(),
                fields,
            });
        }
    }
    structs.sort_by(|left, right| left.name.cmp(&right.name));
    structs
}

struct Typechecker<'a> {
    module: &'a Module,
    ctx: TypeContext,
    diagnostics: Vec<Diagnostic>,
    diagnostic_span_override: Option<Span>,
    scope_depth: usize,
    scope_errors: Vec<Vec<TypeInfo>>,
    task_bindings: Vec<Vec<TaskBinding>>,
    task_value_tail_depth: Option<usize>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TaskBinding {
    name: String,
    scope_depth: usize,
}

impl<'a> Typechecker<'a> {
    fn new(module: &'a Module) -> Self {
        Self {
            module,
            ctx: TypeContext::new(module),
            diagnostics: Vec::new(),
            diagnostic_span_override: None,
            scope_depth: 0,
            scope_errors: Vec::new(),
            task_bindings: Vec::new(),
            task_value_tail_depth: None,
        }
    }

    fn check(mut self) -> TypecheckOutput {
        for item in &self.module.items {
            if let Item::Interface(decl) = item {
                self.check_interface(decl);
            }
        }
        for item in &self.module.items {
            if let Item::Impl(decl) = item {
                self.check_impl(decl);
            }
        }
        for item in &self.module.items {
            match item {
                Item::Function(function) => self.check_function(function),
                Item::Test(test) => {
                    self.check_block(&test.body);
                }
                Item::Struct(decl) => {
                    for field in &decl.fields {
                        if let Some(default) = &field.default {
                            self.infer_expr(default);
                        }
                    }
                }
                Item::Interface(_) | Item::Impl(_) | Item::Enum(_) | Item::Use(_) => {}
            }
        }

        TypecheckOutput {
            diagnostics: self.diagnostics,
        }
    }

    fn check_interface(&mut self, decl: &keelc_ast::InterfaceDecl) {
        if decl.methods.len() > 5 {
            self.diagnostics.push(Diagnostic::error(
                registry::K0601,
                decl.span,
                format!(
                    "interface `{}` declares {} methods; at most 5 are allowed",
                    decl.name.value,
                    decl.methods.len()
                ),
            ));
        }
        for (index, method) in decl.methods.iter().enumerate() {
            if decl.methods[..index]
                .iter()
                .any(|m| m.name.value == method.name.value)
            {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0602,
                    method.span,
                    format!(
                        "duplicate method name `{}` in interface `{}`",
                        method.name.value, decl.name.value
                    ),
                ));
            }
            self.check_method_self(&method.params, method.span);
        }
    }

    fn check_impl(&mut self, decl: &keelc_ast::ImplDecl) {
        let type_name = decl.type_name.value.clone();
        let interface_name = decl.interface_name.value.clone();
        let Some(interface) = self.interface_info(&interface_name).cloned() else {
            self.diagnostics.push(Diagnostic::error(
                registry::K0003,
                decl.interface_name.span,
                format!("unknown interface `{}`", interface_name),
            ));
            return;
        };

        for method in &decl.methods {
            let previous_return = self.ctx.current_return_type().cloned();
            self.ctx.set_current_return_type(
                method
                    .return_type
                    .as_ref()
                    .map_or(TypeInfo::Unit, TypeInfo::from_ast),
            );
            self.push_scope();
            self.define_value(
                &Spanned::new("self".to_string(), method.span),
                TypeInfo::Named(type_name.clone()),
            );
            for param in &method.params {
                if param.name.value == "self" {
                    continue;
                }
                let ty = param
                    .ty
                    .as_ref()
                    .map_or(TypeInfo::Unknown, TypeInfo::from_ast);
                self.define_value(&param.name, ty);
            }
            if let Some(body) = &method.body {
                self.check_block(body);
            }
            self.pop_scope();
            if let Some(return_type) = previous_return {
                self.ctx.set_current_return_type(return_type);
            } else {
                self.ctx.clear_current_return_type();
            }
        }

        for expected in &interface.methods {
            let found = decl.methods.iter().find(|m| m.name.value == expected.name);
            match found {
                None => {
                    self.diagnostics.push(Diagnostic::error(
                        registry::K0603,
                        decl.span,
                        format!(
                            "impl for `{}` on `{}` is missing method `{}`",
                            decl.interface_name.value, decl.type_name.value, expected.name
                        ),
                    ));
                }
                Some(actual) => {
                    let interface_names: Vec<String> = self
                        .ctx
                        .interfaces()
                        .iter()
                        .map(|info| info.name.clone())
                        .collect();
                    let actual_info =
                        keelc_types::infer::method_from_decl(actual, &interface_names);
                    if actual_info.params != expected.params
                        || actual_info.return_type != expected.return_type
                    {
                        self.diagnostics.push(Diagnostic::error(
                            registry::K0604,
                            actual.span,
                            format!(
                                "method `{}` in impl for `{}` on `{}` does not match interface signature",
                                expected.name, decl.interface_name.value, decl.type_name.value
                            ),
                        ));
                    }
                }
            }
        }

        for actual in &decl.methods {
            if !interface
                .methods
                .iter()
                .any(|m| m.name == actual.name.value)
            {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0607,
                    actual.span,
                    format!(
                        "method `{}` is not declared by interface `{}`",
                        actual.name.value, decl.interface_name.value
                    ),
                ));
            }
            self.check_method_self(&actual.params, actual.span);
        }
    }

    fn check_method_self(&mut self, params: &[keelc_ast::Param], span: Span) {
        let Some(first) = params.first() else {
            self.diagnostics.push(Diagnostic::error(
                registry::K0003,
                span,
                "interface methods must declare `self` as the first parameter",
            ));
            return;
        };
        if first.name.value != "self" {
            self.diagnostics.push(Diagnostic::error(
                registry::K0003,
                first.name.span,
                "the first parameter of an interface method must be `self`",
            ));
        }
        if first.ty.is_some() {
            self.diagnostics.push(Diagnostic::error(
                registry::K0003,
                first.name.span,
                "`self` must not have a type annotation",
            ));
        }
    }

    fn check_function(&mut self, function: &keelc_ast::FunctionDecl) {
        let Some(body) = &function.body else {
            return;
        };

        let type_params = keelc_types::type_param_bounds(&function.type_params);
        let previous_return_type = self.ctx.current_return_type().cloned();
        self.ctx
            .set_current_return_type(keelc_types::substitute_type_params(
                &self.resolve_type(
                    &function
                        .return_type
                        .as_ref()
                        .map_or(TypeInfo::Unit, TypeInfo::from_ast),
                ),
                &type_params,
            ));
        self.push_scope();
        for param in &function.params {
            let ty = param
                .ty
                .as_ref()
                .map_or(TypeInfo::Unknown, TypeInfo::from_ast);
            let ty = keelc_types::substitute_type_params(&self.resolve_type(&ty), &type_params);
            self.define_value(&param.name, ty);
        }
        self.check_block(body);
        self.pop_scope();
        if let Some(return_type) = previous_return_type {
            self.ctx.set_current_return_type(return_type);
        } else {
            self.ctx.clear_current_return_type();
        }
    }

    fn check_block(&mut self, block: &Block) -> TypeInfo {
        self.push_scope();
        let mut result = TypeInfo::Unit;
        let mut statements = block.statements.iter().peekable();
        while let Some(statement) = statements.next() {
            let statement_type = self.check_stmt(statement);
            if statements.peek().is_none() && matches!(statement, Stmt::Expr(_)) {
                result = statement_type;
            }
        }
        self.pop_scope();
        result
    }

    fn check_stmt(&mut self, statement: &Stmt) -> TypeInfo {
        match statement {
            Stmt::Let {
                name, ty, value, ..
            } => {
                let value_type = self.infer_expr(value);
                let annotated = ty
                    .as_ref()
                    .map(TypeInfo::from_ast)
                    .map(|ty| self.resolve_type(&ty).clone());
                if let Some(expected) = annotated {
                    self.check_assignable(&value_type, &expected, value.span());
                    self.define_value(name, expected.clone());
                    if task_inner(&expected).is_some() {
                        self.define_task(name);
                    }
                } else {
                    self.define_value(name, value_type.clone());
                    if task_inner(&value_type).is_some() {
                        self.define_task(name);
                    }
                }
                TypeInfo::Unit
            }
            Stmt::Assign { target, value, .. } => {
                self.infer_expr(target);
                self.infer_expr(value);
                TypeInfo::Unit
            }
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    let value_type = self.infer_expr(value);
                    if task_inner(&value_type).is_some() {
                        self.diagnostics.push(Diagnostic::error(
                            registry::K0703,
                            self.diagnostic_span(value.span()),
                            "task handles may not escape their scope",
                        ));
                    }
                }
                TypeInfo::Unit
            }
            Stmt::Assert { value, .. } => {
                self.infer_expr(value);
                TypeInfo::Unit
            }
            Stmt::Expr(expr) => self.infer_expr(expr),
            Stmt::Break(_) | Stmt::Continue(_) => TypeInfo::Unit,
        }
    }

    fn infer_expr(&mut self, expr: &Expr) -> TypeInfo {
        match expr {
            Expr::Missing(_) | Expr::Wildcard(_) => TypeInfo::Unknown,
            Expr::Int(_) => TypeInfo::Int,
            Expr::Float(_) => TypeInfo::Float,
            Expr::String(literal) => {
                self.check_string_interpolations(literal);
                TypeInfo::String
            }
            Expr::Char(_) => TypeInfo::Char,
            Expr::Bool(_) => TypeInfo::Bool,
            Expr::Name(name) => self
                .value_type(&name.value)
                .or_else(|| self.builtin_value_type(&name.value))
                .or_else(|| self.enum_variant_type(&name.value))
                .unwrap_or(TypeInfo::Unknown),
            Expr::Unary { op, expr, .. } => self.infer_unary(*op, expr),
            Expr::Binary {
                left,
                op,
                right,
                span,
            } => self.infer_binary(left, *op, right, *span),
            Expr::Call {
                callee,
                type_args,
                args,
                ..
            } => self.infer_call(callee, type_args, args),
            Expr::Field { target, field, .. } => {
                let target_type = self.infer_expr(target);
                if field.value == "value" {
                    if let Some(inner) = task_inner(&target_type) {
                        if !self.task_value_allowed(target) {
                            self.diagnostics.push(Diagnostic::error(
                                registry::K0702,
                                self.diagnostic_span(field.span),
                                "task result is only readable in the scope tail after the join barrier",
                            ));
                        }
                        return task_value_type(inner);
                    }
                }
                self.ctx
                    .field_type(&target_type, &field.value)
                    .unwrap_or(TypeInfo::Unknown)
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                span,
            } => self.infer_method_call(receiver, method, args, *span),
            Expr::StructLiteral { name, fields, .. } => {
                for field in fields {
                    self.infer_expr(&field.value);
                }
                TypeInfo::Named(name.value.clone())
            }
            Expr::If {
                condition,
                then_block,
                else_branch,
                span,
            } => self.infer_if(condition, then_block, else_branch.as_deref(), *span),
            Expr::Match {
                scrutinee,
                arms,
                span,
            } => {
                let scrutinee_type = self.infer_expr(scrutinee);
                self.check_match_exhaustive(&scrutinee_type, arms, *span);
                let mut result = TypeInfo::Unknown;
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.infer_expr(guard);
                    }
                    let arm_type = self.infer_expr(&arm.value);
                    if result == TypeInfo::Unknown {
                        result = arm_type;
                    }
                }
                result
            }
            Expr::While {
                condition, body, ..
            } => {
                self.infer_expr(condition);
                self.check_block(body);
                TypeInfo::Unit
            }
            Expr::Scope { deadline, body, .. } => self.infer_scope(deadline.as_deref(), body),
            Expr::Spawn { expr, span } => self.infer_spawn(expr, *span),
            Expr::Block(block) => self.check_block(block),
            Expr::Question { expr, span } => self.infer_question(expr, *span),
            Expr::Catch {
                expr,
                error_name,
                arms,
                span,
            } => {
                let result_type = self.infer_expr(expr);
                let (success_type, error_type) = result_type
                    .result_parts()
                    .map_or((TypeInfo::Unknown, TypeInfo::Unknown), |(ok, err)| {
                        (ok.clone(), err.clone())
                    });
                self.check_catch_exhaustive(&error_type, arms, *span);
                self.push_scope();
                self.define_value(error_name, error_type);
                for arm in arms {
                    self.infer_expr(&arm.value);
                }
                self.pop_scope();
                success_type
            }
            Expr::Return { value, .. } => {
                if let Some(value) = value {
                    self.infer_expr(value);
                }
                TypeInfo::Unit
            }
            Expr::Router { routes, span } => self.infer_router(routes, *span),
        }
    }

    fn infer_router(&mut self, routes: &[keelc_ast::Route], anchor: Span) -> TypeInfo {
        for route in routes {
            self.check_http_handler(&route.handler, anchor);
        }
        TypeInfo::Named("http.Router".to_string())
    }

    /// Methods on the compiler-known `std.sql` value types (KDR-0029): pool
    /// query/exec/migrate, `QueryResult.map`/`next`, `RowMapper.collect`.
    fn infer_sql_method(
        &mut self,
        receiver: &TypeInfo,
        method: &Spanned<String>,
        args: &[Expr],
    ) -> Option<TypeInfo> {
        let sql_error = TypeInfo::Named("sql.Error".to_string());
        match receiver {
            TypeInfo::Named(name) if name == "sql.Pool" => {
                let result =
                    |ok: TypeInfo| TypeInfo::generic("Result", vec![ok, sql_error.clone()]);
                Some(match method.value.as_str() {
                    "query" => result(TypeInfo::Named("sql.QueryResult".to_string())),
                    "query_one" => result(TypeInfo::Named("sql.Row".to_string())),
                    "exec" => result(TypeInfo::Int),
                    "migrate" => result(TypeInfo::Unit),
                    _ => return None,
                })
            }
            TypeInfo::Named(name) if name == "sql.QueryResult" => match method.value.as_str() {
                "map" => Some(self.infer_sql_map(args, method.span)),
                "next" => Some(TypeInfo::generic(
                    "Option",
                    vec![TypeInfo::Named("sql.Row".to_string())],
                )),
                _ => None,
            },
            TypeInfo::Generic { name, args: targs } if name == "sql.RowMapper" => {
                match method.value.as_str() {
                    "collect" => {
                        let item = targs.first().cloned().unwrap_or(TypeInfo::Unknown);
                        Some(TypeInfo::generic(
                            "Result",
                            vec![TypeInfo::generic("List", vec![item]), sql_error],
                        ))
                    }
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// `QueryResult.map(f)` — `f` must be a bare function name resolving to
    /// `fn(sql.Row) -> Struct`; `K1506` otherwise. Returns `RowMapper<Struct>`.
    fn infer_sql_map(&mut self, args: &[Expr], span: Span) -> TypeInfo {
        let row = TypeInfo::Named("sql.Row".to_string());
        let item = match args.first() {
            Some(Expr::Name(name)) => match self.function_info(&name.value) {
                Some(info) if info.params == vec![row] => info.return_type.clone(),
                _ => {
                    self.diagnostics.push(Diagnostic::error(
                        registry::K1506,
                        self.diagnostic_span(name.span),
                        format!(
                            "`{}` must be a function `fn(sql.Row) -> Struct`",
                            name.value
                        ),
                    ));
                    TypeInfo::Unknown
                }
            },
            other => {
                let arg_span = other.map_or(span, Expr::span);
                self.diagnostics.push(Diagnostic::error(
                    registry::K1506,
                    self.diagnostic_span(arg_span),
                    "`map` expects a row-mapping function name `fn(sql.Row) -> Struct`",
                ));
                TypeInfo::Unknown
            }
        };
        TypeInfo::Generic {
            name: "sql.RowMapper".to_string(),
            args: vec![item],
        }
    }

    fn infer_scope(&mut self, deadline: Option<&Expr>, body: &Block) -> TypeInfo {
        if let Some(deadline) = deadline {
            let deadline_type = self.infer_expr(deadline);
            if deadline_type != TypeInfo::Named("time.Duration".to_string())
                && deadline_type != TypeInfo::Unknown
            {
                self.diagnostics.push(Diagnostic::error(
                    registry::K1502,
                    self.diagnostic_span(deadline.span()),
                    format!("scope deadline must be `time.Duration`, found `{deadline_type}`"),
                ));
            }
        }

        self.scope_depth += 1;
        self.scope_errors.push(if deadline.is_some() {
            vec![TypeInfo::Named("Cancelled".to_string())]
        } else {
            Vec::new()
        });
        self.push_scope();

        let mut result = TypeInfo::Unit;
        let mut statements = body.statements.iter().peekable();
        while let Some(statement) = statements.next() {
            let is_tail = statements.peek().is_none() && matches!(statement, Stmt::Expr(_));
            if is_tail {
                if let Stmt::Expr(expr) = statement {
                    let previous = self.task_value_tail_depth.replace(self.scope_depth);
                    result = self.infer_expr(expr);
                    self.task_value_tail_depth = previous;
                    if task_inner(&result).is_some() {
                        self.diagnostics.push(Diagnostic::error(
                            registry::K0703,
                            self.diagnostic_span(expr.span()),
                            "task handles may not escape their scope",
                        ));
                    }
                }
            } else {
                self.check_stmt(statement);
            }
        }

        self.pop_scope();
        let errors = self.scope_errors.pop().unwrap_or_default();
        self.scope_depth = self.scope_depth.saturating_sub(1);

        let Some(error_type) = scope_error_type(errors) else {
            return result;
        };
        TypeInfo::generic("Result", vec![result, error_type])
    }

    fn infer_spawn(&mut self, expr: &Expr, span: Span) -> TypeInfo {
        if self.scope_depth == 0 {
            self.diagnostics.push(Diagnostic::error(
                registry::K0701,
                self.diagnostic_span(span),
                "`spawn` is only valid inside a `scope` block",
            ));
        }
        let expr_type = self.infer_expr(expr);
        if let Some((_, error_type)) = expr_type.result_parts() {
            if let Some(errors) = self.scope_errors.last_mut() {
                errors.push(error_type.clone());
            }
        }
        TypeInfo::generic("Task", vec![expr_type])
    }

    fn infer_unary(&mut self, op: keelc_ast::UnaryOp, expr: &Expr) -> TypeInfo {
        let operand_type = self.infer_expr(expr);
        match op {
            keelc_ast::UnaryOp::Negate if operand_type.is_numeric() => operand_type,
            keelc_ast::UnaryOp::Not => TypeInfo::Bool,
            keelc_ast::UnaryOp::Negate => TypeInfo::Unknown,
        }
    }

    fn infer_binary(&mut self, left: &Expr, op: BinaryOp, right: &Expr, span: Span) -> TypeInfo {
        let left_type = self.infer_expr(left);
        let right_type = self.infer_expr(right);
        if is_int_float_pair(&left_type, &right_type) {
            self.diagnostics.push(Diagnostic::error(
                registry::K0202,
                self.diagnostic_span(span),
                format!(
                    "cannot use `{left_type}` and `{right_type}` together without an explicit conversion"
                ),
            ));
            return TypeInfo::Unknown;
        }

        match op {
            BinaryOp::Add
            | BinaryOp::Subtract
            | BinaryOp::Multiply
            | BinaryOp::Divide
            | BinaryOp::Remainder => {
                if left_type == right_type && left_type.is_numeric() {
                    left_type
                } else {
                    TypeInfo::Unknown
                }
            }
            BinaryOp::Equal
            | BinaryOp::NotEqual
            | BinaryOp::Less
            | BinaryOp::LessEqual
            | BinaryOp::Greater
            | BinaryOp::GreaterEqual
            | BinaryOp::And
            | BinaryOp::Or => TypeInfo::Bool,
        }
    }

    fn infer_call(
        &mut self,
        callee: &Expr,
        type_args: &[keelc_ast::Type],
        args: &[Expr],
    ) -> TypeInfo {
        let arg_types: Vec<TypeInfo> = args.iter().map(|arg| self.infer_expr(arg)).collect();

        match callee {
            Expr::Name(name) if name.value == "print" => TypeInfo::Unit,
            Expr::Name(name) if name.value == "checked_div" || name.value == "checked_rem" => {
                TypeInfo::Generic {
                    name: "Option".to_string(),
                    args: vec![TypeInfo::Int],
                }
            }
            Expr::Name(name) if name.value == "check_cancel" => TypeInfo::generic(
                "Result",
                vec![TypeInfo::Unit, TypeInfo::Named("Cancelled".to_string())],
            ),
            Expr::Name(name) if name.value == "Some" => TypeInfo::Generic {
                name: "Option".to_string(),
                args: vec![arg_types.first().cloned().unwrap_or(TypeInfo::Unknown)],
            },
            Expr::Name(name) if name.value == "Ok" => TypeInfo::Generic {
                name: "Result".to_string(),
                args: vec![
                    arg_types.first().cloned().unwrap_or(TypeInfo::Unknown),
                    TypeInfo::Unknown,
                ],
            },
            Expr::Name(name) if name.value == "Err" => TypeInfo::Generic {
                name: "Result".to_string(),
                args: vec![
                    TypeInfo::Unknown,
                    arg_types.first().cloned().unwrap_or(TypeInfo::Unknown),
                ],
            },
            Expr::Name(name) => {
                if let Some(info) = self.function_info(&name.value) {
                    let params = info.params.clone();
                    let return_type = info.return_type.clone();
                    self.check_call_args(&params, args, name.span);
                    return return_type;
                }
                self.enum_variant_type(&name.value)
                    .unwrap_or(TypeInfo::Unknown)
            }
            Expr::Field { target, field, .. }
                if matches!(target.as_ref(), Expr::Name(name) if name.value == "Float")
                    && field.value == "from" =>
            {
                self.check_call_args(&[TypeInfo::Int], args, field.span);
                TypeInfo::Float
            }
            Expr::Field { target, field, .. }
                if matches!(target.as_ref(), Expr::Name(name) if name.value == "json")
                    && field.value == "parse" =>
            {
                self.check_call_args(&[TypeInfo::String], &args[..args.len().min(1)], field.span);
                let target_type = type_args
                    .first()
                    .map(TypeInfo::from_ast)
                    .map(|ty| self.resolve_type(&ty))
                    .unwrap_or(TypeInfo::Unknown);
                if type_args.len() != 1 || !self.ctx.is_json_representable(&target_type) {
                    let span = type_args.first().map_or(field.span, keelc_ast::Type::span);
                    self.diagnostics.push(Diagnostic::error(
                        registry::K1503,
                        self.diagnostic_span(span),
                        format!("type `{target_type}` is not JSON-representable"),
                    ));
                }
                TypeInfo::generic(
                    "Result",
                    vec![target_type, TypeInfo::Named("json.Error".to_string())],
                )
            }
            Expr::Field { target, field, .. } if matches!(target.as_ref(), Expr::Name(name) if name.value == "http") => {
                self.infer_http_call(field, args)
            }
            Expr::Field { target, field, .. } if matches!(target.as_ref(), Expr::Name(name) if name.value == "log") => {
                match field.value.as_str() {
                    "info" | "warn" | "error" => {
                        self.check_call_args(&[TypeInfo::String], args, field.span);
                        TypeInfo::Unit
                    }
                    _ => {
                        self.diagnostics.push(Diagnostic::error(
                            registry::K0606,
                            self.diagnostic_span(field.span),
                            format!("method `{}` not found on `std.log`", field.value),
                        ));
                        TypeInfo::Unknown
                    }
                }
            }
            Expr::Field { target, field, .. } if matches!(target.as_ref(), Expr::Name(name) if name.value == "sql") => {
                match field.value.as_str() {
                    "connect" => {
                        self.check_call_args(&[TypeInfo::String], args, field.span);
                        TypeInfo::generic(
                            "Result",
                            vec![
                                TypeInfo::Named("sql.Pool".to_string()),
                                TypeInfo::Named("sql.Error".to_string()),
                            ],
                        )
                    }
                    _ => {
                        self.diagnostics.push(Diagnostic::error(
                            registry::K0606,
                            self.diagnostic_span(field.span),
                            format!("method `{}` not found on `std.sql`", field.value),
                        ));
                        TypeInfo::Unknown
                    }
                }
            }
            Expr::Field { target, field, .. }
                if field.value == "get"
                    && self.infer_expr(target) == TypeInfo::Named("sql.Row".to_string()) =>
            {
                self.check_call_args(&[TypeInfo::Int], args, field.span);
                type_args
                    .first()
                    .map(TypeInfo::from_ast)
                    .map(|ty| self.resolve_type(&ty))
                    .unwrap_or(TypeInfo::Unknown)
            }
            Expr::Field { target, field, .. }
                if matches!(field.value.as_str(), "path_param" | "query_param") =>
            {
                let target_type = self.infer_expr(target);
                if target_type == TypeInfo::Named("http.Request".to_string()) {
                    self.check_call_args(&[TypeInfo::String], args, field.span);
                    let parsed = type_args
                        .first()
                        .map(TypeInfo::from_ast)
                        .map(|ty| self.resolve_type(&ty))
                        .unwrap_or(TypeInfo::Unknown);
                    match field.value.as_str() {
                        "path_param" => TypeInfo::generic("Result", vec![parsed, TypeInfo::String]),
                        _ => TypeInfo::generic("Option", vec![parsed]),
                    }
                } else {
                    TypeInfo::Unknown
                }
            }
            Expr::Field { .. } => {
                self.infer_expr(callee);
                TypeInfo::Unknown
            }
            _ => {
                self.infer_expr(callee);
                TypeInfo::Unknown
            }
        }
    }

    fn check_call_args(&mut self, params: &[TypeInfo], args: &[Expr], span: Span) {
        for (index, (param, arg)) in params.iter().zip(args.iter()).enumerate() {
            let arg_type = self.infer_expr(arg);
            self.check_assignable(&arg_type, param, arg.span());
            let _ = index;
        }
        let _ = (params, args, span);
    }

    fn infer_method_call(
        &mut self,
        receiver: &Expr,
        method: &Spanned<String>,
        args: &[Expr],
        span: Span,
    ) -> TypeInfo {
        if matches!(receiver, Expr::Name(name) if name.value == "Float") && method.value == "from" {
            self.check_call_args(&[TypeInfo::Int], args, method.span);
            return TypeInfo::Float;
        }
        if matches!(receiver, Expr::Name(name) if name.value == "time") {
            return match method.value.as_str() {
                "milliseconds" | "seconds" => {
                    self.check_call_args(&[TypeInfo::Int], args, method.span);
                    TypeInfo::Named("time.Duration".to_string())
                }
                "sleep" => {
                    self.check_call_args(
                        &[TypeInfo::Named("time.Duration".to_string())],
                        args,
                        method.span,
                    );
                    TypeInfo::generic(
                        "Result",
                        vec![TypeInfo::Unit, TypeInfo::Named("Cancelled".to_string())],
                    )
                }
                _ => {
                    self.diagnostics.push(Diagnostic::error(
                        registry::K0606,
                        self.diagnostic_span(span),
                        format!("method `{}` not found on `std.time`", method.value),
                    ));
                    TypeInfo::Unknown
                }
            };
        }
        if matches!(receiver, Expr::Name(name) if name.value == "http") {
            return self.infer_http_call(method, args);
        }
        if matches!(receiver, Expr::Name(name) if name.value == "sql") {
            return match method.value.as_str() {
                "connect" => {
                    self.check_call_args(&[TypeInfo::String], args, method.span);
                    TypeInfo::generic(
                        "Result",
                        vec![
                            TypeInfo::Named("sql.Pool".to_string()),
                            TypeInfo::Named("sql.Error".to_string()),
                        ],
                    )
                }
                _ => {
                    self.diagnostics.push(Diagnostic::error(
                        registry::K0606,
                        self.diagnostic_span(method.span),
                        format!("method `{}` not found on `std.sql`", method.value),
                    ));
                    TypeInfo::Unknown
                }
            };
        }
        if matches!(receiver, Expr::Name(name) if name.value == "log") {
            return match method.value.as_str() {
                "info" | "warn" | "error" => {
                    self.check_call_args(&[TypeInfo::String], args, method.span);
                    TypeInfo::Unit
                }
                _ => {
                    self.diagnostics.push(Diagnostic::error(
                        registry::K0606,
                        self.diagnostic_span(method.span),
                        format!("method `{}` not found on `std.log`", method.value),
                    ));
                    TypeInfo::Unknown
                }
            };
        }
        if matches!(receiver, Expr::Name(name) if name.value == "json") && method.value == "write" {
            let value_type = args
                .first()
                .map(|arg| self.infer_expr(arg))
                .unwrap_or(TypeInfo::Unknown);
            if !self.ctx.is_json_representable(&value_type) {
                self.diagnostics.push(Diagnostic::error(
                    registry::K1503,
                    self.diagnostic_span(args.first().map_or(method.span, Expr::span)),
                    format!("type `{value_type}` is not JSON-representable"),
                ));
            }
            return TypeInfo::generic(
                "Result",
                vec![TypeInfo::String, TypeInfo::Named("json.Error".to_string())],
            );
        }
        let receiver_type = self.infer_expr(receiver);
        for arg in args {
            self.infer_expr(arg);
        }
        if let Some(result) = self.infer_sql_method(&receiver_type, method, args) {
            return result;
        }
        let method_info = match &receiver_type {
            TypeInfo::Interface(name) | TypeInfo::TypeParam { bound: name, .. } => {
                let interface = match self.interface_info(name) {
                    Some(info) => info,
                    None => return TypeInfo::Unknown,
                };
                interface
                    .methods
                    .iter()
                    .find(|m| m.name == method.value)
                    .cloned()
            }
            TypeInfo::Named(type_name) => self
                .ctx
                .impls()
                .iter()
                .filter(|info| info.type_name == *type_name)
                .flat_map(|info| info.methods.iter())
                .find(|m| m.name == method.value)
                .cloned(),
            _ => None,
        };
        match method_info {
            Some(info) => {
                self.check_call_args(&info.params, args, method.span);
                info.return_type
            }
            None => {
                if let TypeInfo::TypeParam { name, bound } = &receiver_type {
                    self.diagnostics.push(Diagnostic::error(
                        registry::K0802,
                        self.diagnostic_span(span),
                        format!(
                            "method `{}` is not declared by the bound `{bound}` of type parameter `{name}`",
                            method.value
                        ),
                    ));
                } else {
                    self.diagnostics.push(Diagnostic::error(
                        registry::K0606,
                        self.diagnostic_span(span),
                        format!("method `{}` not found on `{}`", method.value, receiver_type),
                    ));
                }
                TypeInfo::Unknown
            }
        }
    }

    fn infer_if(
        &mut self,
        condition: &Expr,
        then_block: &Block,
        else_branch: Option<&Expr>,
        span: Span,
    ) -> TypeInfo {
        self.infer_expr(condition);
        let then_type = self.check_block(then_block);
        let Some(else_branch) = else_branch else {
            return TypeInfo::Unit;
        };
        let else_type = self.infer_expr(else_branch);
        if !types_compatible(&then_type, &else_type) {
            self.diagnostics.push(Diagnostic::error(
                registry::K0401,
                self.diagnostic_span(span),
                format!("if arms have incompatible types `{then_type}` and `{else_type}`"),
            ));
            TypeInfo::Unknown
        } else {
            merge_types(&then_type, &else_type)
        }
    }

    fn infer_http_call(&mut self, field: &Spanned<String>, args: &[Expr]) -> TypeInfo {
        match field.value.as_str() {
            "ok" | "created" | "bad_request" | "conflict" | "internal_error" => {
                self.check_call_args(&[TypeInfo::String], args, field.span);
                TypeInfo::Named("http.Response".to_string())
            }
            "no_content" | "not_found" => TypeInfo::Named("http.Response".to_string()),
            "serve" => {
                if let Some(port) = args.first() {
                    let port_type = self.infer_expr(port);
                    self.check_assignable(&port_type, &TypeInfo::Int, port.span());
                }
                if let Some(routes) = args.get(1) {
                    let routes_type = self.infer_expr(routes);
                    self.check_assignable(
                        &routes_type,
                        &TypeInfo::Named("http.Router".to_string()),
                        routes.span(),
                    );
                }
                TypeInfo::generic(
                    "Result",
                    vec![TypeInfo::Unit, TypeInfo::Named("http.Error".to_string())],
                )
            }
            _ => {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0606,
                    self.diagnostic_span(field.span),
                    format!("method `{}` not found on `std.http`", field.value),
                ));
                TypeInfo::Unknown
            }
        }
    }

    fn check_http_handler(&mut self, handler: &RouteHandler, anchor: Span) {
        let request = TypeInfo::Named("http.Request".to_string());
        let response = TypeInfo::Named("http.Response".to_string());
        match handler {
            RouteHandler::Closure { param, body, .. } => {
                self.push_scope();
                self.define_value(param, request);
                let body_type = self.infer_expr(body);
                self.pop_scope();
                if !types_compatible(&body_type, &response) {
                    self.diagnostics.push(Diagnostic::error(
                        registry::K1504,
                        self.diagnostic_span(anchor),
                        format!(
                            "route handler closure returns `{body_type}`, expected `http.Response`"
                        ),
                    ));
                }
            }
            RouteHandler::Expr(expr) => {
                let Expr::Name(name) = expr.as_ref() else {
                    self.diagnostics.push(Diagnostic::error(
                        registry::K1504,
                        self.diagnostic_span(anchor),
                        "route handler must be a function name `fn(http.Request) -> http.Response` or a `fn(req) => ...` closure",
                    ));
                    return;
                };
                let Some(info) = self.function_info(&name.value) else {
                    self.diagnostics.push(Diagnostic::error(
                        registry::K1504,
                        self.diagnostic_span(anchor),
                        format!("`{}` is not a function", name.value),
                    ));
                    return;
                };
                if info.params != vec![request] || info.return_type != response {
                    self.diagnostics.push(Diagnostic::error(
                        registry::K1504,
                        self.diagnostic_span(anchor),
                        format!(
                            "`{}` has signature `fn({}) -> {}`, expected `fn(http.Request) -> http.Response`",
                            name.value,
                            info.params.iter().map(ToString::to_string).collect::<Vec<_>>().join(", "),
                            info.return_type,
                        ),
                    ));
                }
            }
        }
    }

    fn infer_question(&mut self, expr: &Expr, span: Span) -> TypeInfo {
        let expr_type = self.infer_expr(expr);
        match &expr_type {
            TypeInfo::Generic { name, args } if name == "Result" && args.len() == 2 => {
                let (Some(success_type), Some(error_type)) = (args.first(), args.get(1)) else {
                    return TypeInfo::Unknown;
                };
                let can_absorb = self
                    .ctx
                    .current_return_type()
                    .and_then(|ty| ty.result_parts())
                    .is_some_and(|(_, return_error)| type_absorbs(return_error, error_type));
                if can_absorb {
                    success_type.clone()
                } else {
                    self.report_question_context(span, &expr_type);
                    TypeInfo::Unknown
                }
            }
            TypeInfo::Generic { name, args } if name == "Option" && args.len() == 1 => {
                let Some(success_type) = args.first() else {
                    return TypeInfo::Unknown;
                };
                let can_absorb = self
                    .ctx
                    .current_return_type()
                    .and_then(|ty| ty.option_inner())
                    .is_some();
                if can_absorb {
                    success_type.clone()
                } else {
                    self.report_question_context(span, &expr_type);
                    TypeInfo::Unknown
                }
            }
            _ => {
                self.report_question_context(span, &expr_type);
                TypeInfo::Unknown
            }
        }
    }

    fn report_question_context(&mut self, span: Span, expr_type: &TypeInfo) {
        let return_type = self
            .ctx
            .current_return_type()
            .map_or_else(|| TypeInfo::Unit.to_string(), ToString::to_string);
        self.diagnostics.push(Diagnostic::error(
            registry::K0501,
            self.diagnostic_span(span),
            format!("`?` cannot unwrap `{expr_type}` in a function returning `{return_type}`"),
        ));
    }

    fn check_exhaustive(
        &mut self,
        scrutinee_type: &TypeInfo,
        arms: &[MatchArm],
        span: Span,
        has_fallback: fn(&MatchArm) -> bool,
        code: keelc_diag::Code,
        message: &str,
    ) {
        if arms.iter().any(has_fallback) {
            return;
        }
        let Some(variants) = self.exhaustive_variants(scrutinee_type) else {
            return;
        };

        if let Some(missing) = variants.iter().find(|variant| {
            !arms
                .iter()
                .any(|arm| arm.guard.is_none() && arm_pattern_name(arm) == Some(variant.as_str()))
        }) {
            self.diagnostics.push(Diagnostic::error(
                code,
                self.diagnostic_span(span),
                format!("{message}; missing `{missing}`"),
            ));
        }
    }

    fn check_match_exhaustive(&mut self, scrutinee_type: &TypeInfo, arms: &[MatchArm], span: Span) {
        self.check_exhaustive(
            scrutinee_type,
            arms,
            span,
            is_unguarded_wildcard_arm,
            registry::K0402,
            "match is not exhaustive",
        );

        // K0403: warn on wildcard `_` against a same-module enum
        if let TypeInfo::Named(name) = scrutinee_type {
            if self.ctx.enums().iter().any(|info| info.name == *name) {
                if let Some(arm) = arms
                    .iter()
                    .find(|arm| arm.guard.is_none() && matches!(arm.pattern, Pattern::Wildcard(_)))
                {
                    self.diagnostics.push(Diagnostic::warning(
                        registry::K0403,
                        arm.pattern.span(),
                        format!(
                            "wildcard `_` matches all variants of `{name}`; prefer naming variants explicitly"
                        ),
                    ));
                }
            }
        }
    }

    fn check_catch_exhaustive(&mut self, error_type: &TypeInfo, arms: &[MatchArm], span: Span) {
        let code = if matches!(error_type, TypeInfo::Union(_)) {
            registry::K0503
        } else {
            registry::K0502
        };
        let message = if matches!(error_type, TypeInfo::Union(_)) {
            "union error match is not exhaustive"
        } else {
            "catch is not exhaustive"
        };
        self.check_exhaustive(error_type, arms, span, is_catch_fallback_arm, code, message);
    }

    fn exhaustive_variants(&self, ty: &TypeInfo) -> Option<Vec<String>> {
        self.ctx.exhaustive_variants(ty)
    }

    fn builtin_value_type(&self, name: &str) -> Option<TypeInfo> {
        self.ctx.builtin_value_type(name)
    }

    fn enum_variant_type(&self, variant_name: &str) -> Option<TypeInfo> {
        self.ctx.enum_variant_type(variant_name)
    }

    fn check_string_interpolations(&mut self, literal: &Spanned<StringLiteral>) {
        for interpolation in &literal.value.interpolations {
            let Some(expr) = keelc_parse::parse_interpolation_expr(
                interpolation.span.source,
                &interpolation.value,
            ) else {
                continue;
            };
            let previous_override = self.diagnostic_span_override.replace(interpolation.span);
            self.infer_expr(&expr);
            self.diagnostic_span_override = previous_override;
        }
    }

    fn define_value(&mut self, name: &Spanned<String>, ty: TypeInfo) {
        self.ctx.define_value(&name.value, ty);
    }

    fn define_task(&mut self, name: &Spanned<String>) {
        if let Some(scope) = self.task_bindings.last_mut() {
            scope.push(TaskBinding {
                name: name.value.clone(),
                scope_depth: self.scope_depth,
            });
        }
    }

    fn value_type(&self, name: &str) -> Option<TypeInfo> {
        self.ctx.value_type(name)
    }

    fn resolve_type(&self, ty: &TypeInfo) -> TypeInfo {
        self.ctx.resolve_type(ty)
    }

    fn check_assignable(&mut self, actual: &TypeInfo, expected: &TypeInfo, span: Span) {
        if matches!(actual, TypeInfo::Unknown) || matches!(expected, TypeInfo::Unknown) {
            return;
        }
        if expected == actual {
            return;
        }
        if let TypeInfo::Interface(interface_name) = expected {
            if let TypeInfo::Named(type_name) = actual {
                if self.impl_exists(type_name, interface_name) {
                    return;
                }
            }
            self.diagnostics.push(Diagnostic::error(
                registry::K0605,
                self.diagnostic_span(span),
                format!("type `{actual}` does not implement interface `{expected}`"),
            ));
            return;
        }
        if let TypeInfo::TypeParam { name, bound } = expected {
            // A type argument flowing into a type-parameter slot must satisfy
            // the parameter's interface bound structurally (§8.3).
            if let TypeInfo::TypeParam {
                name: actual_name, ..
            } = actual
            {
                if actual_name == name {
                    return;
                }
            }
            let missing = self.bound_methods_missing_on(actual, bound);
            if !missing.is_empty() {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0803,
                    self.diagnostic_span(span),
                    format!(
                        "type `{actual}` does not satisfy bound `{bound}` of type parameter `{name}`; missing {}",
                        missing
                            .iter()
                            .map(|m| format!("`{m}`"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                ));
            }
            return;
        }
        if let (
            TypeInfo::Generic {
                name: actual_name,
                args: actual_args,
            },
            TypeInfo::Generic {
                name: expected_name,
                args: expected_args,
            },
        ) = (actual, expected)
        {
            if actual_name == expected_name && actual_args.len() == expected_args.len() {
                for (actual_arg, expected_arg) in actual_args.iter().zip(expected_args.iter()) {
                    self.check_assignable(actual_arg, expected_arg, span);
                }
                return;
            }
        }
        if actual != expected {
            self.diagnostics.push(Diagnostic::error(
                registry::K0003,
                self.diagnostic_span(span),
                format!("expected `{expected}`, found `{actual}`"),
            ));
        }
    }

    fn impl_exists(&self, type_name: &str, interface_name: &str) -> bool {
        self.ctx
            .impls()
            .iter()
            .any(|info| info.type_name == type_name && info.interface_name == interface_name)
    }

    /// Names of the methods declared by `bound` that `actual` does not provide
    /// structurally — i.e. no `impl` on the type supplies a method with a
    /// matching name, parameter types, and return type (§8.3). An empty result
    /// means the bound is satisfied.
    fn bound_methods_missing_on(&self, actual: &TypeInfo, bound: &str) -> Vec<String> {
        let Some(interface) = self.interface_info(bound) else {
            return Vec::new();
        };
        let Some(type_name) = type_satisfaction_name(actual) else {
            // Unknown/unresolved receiver — defer rather than over-report.
            return Vec::new();
        };
        let methods = interface.methods.clone();
        methods
            .iter()
            .filter(|required| !self.type_has_matching_method(type_name, required))
            .map(|required| required.name.clone())
            .collect()
    }

    fn type_has_matching_method(
        &self,
        type_name: &str,
        required: &keelc_types::infer::MethodInfo,
    ) -> bool {
        self.ctx
            .impls()
            .iter()
            .filter(|info| info.type_name == type_name)
            .flat_map(|info| info.methods.iter())
            .any(|candidate| {
                candidate.name == required.name
                    && candidate.params == required.params
                    && candidate.return_type == required.return_type
            })
    }

    fn interface_info(&self, name: &str) -> Option<&keelc_types::infer::InterfaceInfo> {
        self.ctx.interface_info(name)
    }

    fn function_info(&self, name: &str) -> Option<&keelc_types::infer::FunctionInfo> {
        self.ctx.function_info(name)
    }

    fn diagnostic_span(&self, fallback: Span) -> Span {
        self.diagnostic_span_override.unwrap_or(fallback)
    }

    fn push_scope(&mut self) {
        self.ctx.push_scope();
        self.task_bindings.push(Vec::new());
    }

    fn pop_scope(&mut self) {
        self.ctx.pop_scope();
        let _ = self.task_bindings.pop();
    }

    fn task_value_allowed(&self, target: &Expr) -> bool {
        let Expr::Name(name) = target else {
            return false;
        };
        let Some(tail_depth) = self.task_value_tail_depth else {
            return false;
        };
        self.task_bindings
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev())
            .find(|binding| binding.name == name.value)
            .is_some_and(|binding| binding.scope_depth == tail_depth)
    }
}

/// The `impl` type name used to look up methods for a concrete type during
/// structural constraint satisfaction. Primitives carry built-in impls keyed
/// by their spelled name (`Int`, `Float`, ...).
fn type_satisfaction_name(ty: &TypeInfo) -> Option<&str> {
    match ty {
        TypeInfo::Named(name) => Some(name),
        TypeInfo::Int => Some("Int"),
        TypeInfo::Float => Some("Float"),
        TypeInfo::Bool => Some("Bool"),
        TypeInfo::String => Some("String"),
        TypeInfo::Char => Some("Char"),
        _ => None,
    }
}

fn is_unguarded_wildcard_arm(arm: &MatchArm) -> bool {
    arm.guard.is_none() && matches!(arm.pattern, Pattern::Wildcard(_))
}

fn is_catch_fallback_arm(arm: &MatchArm) -> bool {
    if arm.guard.is_some() {
        return false;
    }
    match &arm.pattern {
        Pattern::Wildcard(_) => true,
        Pattern::Name { name, args, .. } => name.value == "other" && args.is_empty(),
    }
}

fn arm_pattern_name(arm: &MatchArm) -> Option<&str> {
    match &arm.pattern {
        Pattern::Name { name, .. } => Some(name.value.as_str()),
        Pattern::Wildcard(_) => None,
    }
}

fn task_inner(ty: &TypeInfo) -> Option<&TypeInfo> {
    match ty {
        TypeInfo::Generic { name, args } if name == "Task" && args.len() == 1 => args.first(),
        _ => None,
    }
}

fn task_value_type(inner: &TypeInfo) -> TypeInfo {
    inner
        .result_parts()
        .map_or_else(|| inner.clone(), |(ok, _)| ok.clone())
}

fn scope_error_type(errors: Vec<TypeInfo>) -> Option<TypeInfo> {
    let mut unique = Vec::new();
    for error in errors {
        if !unique.iter().any(|seen| seen == &error) {
            unique.push(error);
        }
    }
    reduce_error_types(unique)
}

#[cfg(test)]
mod tests {
    use super::{resolve, typecheck};
    use keelc_diag::registry;
    use keelc_parse::parse;
    use keelc_span::SourceId;

    #[test]
    fn reports_assignment_to_immutable_let() {
        let output = parse(SourceId::new(0), "fn main() {\nlet x = 1\nx = 2\n}\n");
        assert!(output.diagnostics.is_empty());

        let resolved = resolve(&output.module);

        assert_eq!(resolved.diagnostics[0].code, registry::K0303);
    }

    #[test]
    fn allows_assignment_to_mut_binding() {
        let output = parse(SourceId::new(0), "fn main() {\nmut x = 1\nx = 2\n}\n");
        assert!(output.diagnostics.is_empty());

        let resolved = resolve(&output.module);

        assert!(resolved.diagnostics.is_empty());
    }

    #[test]
    fn reports_missing_required_struct_field() {
        let output = parse(
            SourceId::new(0),
            "struct User {\nid: Int\nname: String\n}\nfn main() {\nlet u = User{ id: 1 }\n}\n",
        );
        assert!(output.diagnostics.is_empty());

        let resolved = resolve(&output.module);

        assert_eq!(resolved.diagnostics[0].code, registry::K0301);
    }

    #[test]
    fn permits_missing_struct_field_with_default() {
        let output = parse(
            SourceId::new(0),
            "struct Config {\nhost: String\nport: Int = 8080\n}\nfn main() {\nlet c = Config{ host: \"localhost\" }\n}\n",
        );
        assert!(output.diagnostics.is_empty());

        let resolved = resolve(&output.module);

        assert!(resolved.diagnostics.is_empty());
    }

    #[test]
    fn reports_int_float_arithmetic() {
        let output = parse(SourceId::new(0), "fn main() {\nlet value = 1 + 2.5\n}\n");
        assert!(output.diagnostics.is_empty());

        let checked = typecheck(&output.module);

        assert_eq!(
            checked
                .diagnostics
                .first()
                .map(|diagnostic| diagnostic.code),
            Some(registry::K0202)
        );
    }

    #[test]
    fn reports_int_float_equality() {
        let output = parse(SourceId::new(0), "fn main() {\nif 1 == 1.0 {\n}\n}\n");
        assert!(output.diagnostics.is_empty());

        let checked = typecheck(&output.module);

        assert_eq!(
            checked
                .diagnostics
                .first()
                .map(|diagnostic| diagnostic.code),
            Some(registry::K0202)
        );
    }

    #[test]
    fn reports_int_float_in_interpolation() {
        let output = parse(
            SourceId::new(0),
            "fn main() {\nlet int_value = 1\nlet float_value = 2.0\nprint(\"{int_value + float_value}\")\n}\n",
        );
        assert!(output.diagnostics.is_empty());

        let checked = typecheck(&output.module);

        assert_eq!(
            checked
                .diagnostics
                .first()
                .map(|diagnostic| diagnostic.code),
            Some(registry::K0202)
        );
    }

    #[test]
    fn reports_if_arm_type_mismatch() {
        let output = parse(
            SourceId::new(0),
            "fn main() {\nlet value = if true { 1 } else { \"one\" }\n}\n",
        );
        assert!(output.diagnostics.is_empty());

        let checked = typecheck(&output.module);

        assert_eq!(
            checked
                .diagnostics
                .first()
                .map(|diagnostic| diagnostic.code),
            Some(registry::K0401)
        );
    }

    #[test]
    fn permits_matching_if_arm_types() {
        let output = parse(
            SourceId::new(0),
            "fn main() {\nlet value = if true { \"one\" } else { \"two\" }\nprint(value)\n}\n",
        );
        assert!(output.diagnostics.is_empty());

        let checked = typecheck(&output.module);

        assert!(checked.diagnostics.is_empty());
    }

    #[test]
    fn ignores_escaped_brace_text_when_checking_interpolations() {
        let output = parse(SourceId::new(0), "fn main() {\nprint(\"{{1 + 2.0}}\")\n}\n");
        assert!(output.diagnostics.is_empty());

        let checked = typecheck(&output.module);

        assert!(checked.diagnostics.is_empty());
    }
}

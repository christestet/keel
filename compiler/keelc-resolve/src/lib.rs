//! Name resolution and early semantic diagnostics for Keel Core.

use keelc_ast::{BinaryOp, Block, Expr, Item, Module, Stmt, StringLiteral, Type as AstType};
use keelc_diag::{registry, Diagnostic};
use keelc_parse::parse;
use keelc_span::{Span, Spanned};
use std::fmt;

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
                Item::Struct(_) | Item::Enum(_) | Item::Use(_) => {}
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

#[derive(Clone, Debug, Eq, PartialEq)]
struct FunctionInfo {
    name: String,
    return_type: TypeInfo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TypedBinding {
    name: String,
    ty: TypeInfo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TypeInfo {
    Int,
    Float,
    Bool,
    String,
    Char,
    Unit,
    Named(String),
    Generic { name: String, args: Vec<TypeInfo> },
    Union(Vec<TypeInfo>),
    Unknown,
}

impl TypeInfo {
    fn from_ast(ty: &AstType) -> Self {
        match ty {
            AstType::Named { name, args, .. } if args.is_empty() => match name.value.as_str() {
                "Int" => Self::Int,
                "Float" => Self::Float,
                "Bool" => Self::Bool,
                "String" => Self::String,
                "Char" => Self::Char,
                "Unit" => Self::Unit,
                _ => Self::Named(name.value.clone()),
            },
            AstType::Named { name, args, .. } => Self::Generic {
                name: name.value.clone(),
                args: args.iter().map(Self::from_ast).collect(),
            },
            AstType::Union { members, .. } => {
                Self::Union(members.iter().map(Self::from_ast).collect())
            }
        }
    }

    const fn is_known(&self) -> bool {
        !matches!(self, Self::Unknown)
    }

    const fn is_numeric(&self) -> bool {
        matches!(self, Self::Int | Self::Float)
    }
}

impl fmt::Display for TypeInfo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int => formatter.write_str("Int"),
            Self::Float => formatter.write_str("Float"),
            Self::Bool => formatter.write_str("Bool"),
            Self::String => formatter.write_str("String"),
            Self::Char => formatter.write_str("Char"),
            Self::Unit => formatter.write_str("Unit"),
            Self::Named(name) => formatter.write_str(name),
            Self::Generic { name, args } => {
                write!(formatter, "{name}<")?;
                write_type_list(formatter, args, ", ")?;
                formatter.write_str(">")
            }
            Self::Union(members) => write_type_list(formatter, members, " | "),
            Self::Unknown => formatter.write_str("<unknown>"),
        }
    }
}

fn write_type_list(
    formatter: &mut fmt::Formatter<'_>,
    types: &[TypeInfo],
    separator: &str,
) -> fmt::Result {
    let mut first = true;
    for ty in types {
        if first {
            first = false;
        } else {
            formatter.write_str(separator)?;
        }
        write!(formatter, "{ty}")?;
    }
    Ok(())
}

struct Typechecker<'a> {
    module: &'a Module,
    functions: Vec<FunctionInfo>,
    scopes: Vec<Vec<TypedBinding>>,
    diagnostics: Vec<Diagnostic>,
    diagnostic_span_override: Option<Span>,
}

impl<'a> Typechecker<'a> {
    fn new(module: &'a Module) -> Self {
        Self {
            module,
            functions: collect_functions(module),
            scopes: Vec::new(),
            diagnostics: Vec::new(),
            diagnostic_span_override: None,
        }
    }

    fn check(mut self) -> TypecheckOutput {
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
                Item::Enum(_) | Item::Use(_) => {}
            }
        }

        TypecheckOutput {
            diagnostics: self.diagnostics,
        }
    }

    fn check_function(&mut self, function: &keelc_ast::FunctionDecl) {
        let Some(body) = &function.body else {
            return;
        };

        self.push_scope();
        for param in &function.params {
            let ty = param
                .ty
                .as_ref()
                .map_or(TypeInfo::Unknown, TypeInfo::from_ast);
            self.define_value(&param.name, ty);
        }
        self.check_block(body);
        self.pop_scope();
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
            Stmt::Let { name, value, .. } => {
                let value_type = self.infer_expr(value);
                self.define_value(name, value_type);
                TypeInfo::Unit
            }
            Stmt::Assign { target, value, .. } => {
                self.infer_expr(target);
                self.infer_expr(value);
                TypeInfo::Unit
            }
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.infer_expr(value);
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
            Expr::Name(name) => self.value_type(&name.value).unwrap_or(TypeInfo::Unknown),
            Expr::Unary { op, expr, .. } => self.infer_unary(*op, expr),
            Expr::Binary {
                left,
                op,
                right,
                span,
            } => self.infer_binary(left, *op, right, *span),
            Expr::Call { callee, args, .. } => self.infer_call(callee, args),
            Expr::Field { target, .. } => {
                self.infer_expr(target);
                TypeInfo::Unknown
            }
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
                scrutinee, arms, ..
            } => {
                self.infer_expr(scrutinee);
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
            Expr::Block(block) => self.check_block(block),
            Expr::Question { expr, .. } => {
                self.infer_expr(expr);
                TypeInfo::Unknown
            }
            Expr::Catch { expr, arms, .. } => {
                self.infer_expr(expr);
                for arm in arms {
                    self.infer_expr(&arm.value);
                }
                TypeInfo::Unknown
            }
            Expr::Return { value, .. } => {
                if let Some(value) = value {
                    self.infer_expr(value);
                }
                TypeInfo::Unit
            }
        }
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

    fn infer_call(&mut self, callee: &Expr, args: &[Expr]) -> TypeInfo {
        for arg in args {
            self.infer_expr(arg);
        }

        match callee {
            Expr::Name(name) if name.value == "print" => TypeInfo::Unit,
            Expr::Name(name) => self
                .function_return_type(&name.value)
                .unwrap_or(TypeInfo::Unknown),
            Expr::Field { target, field, .. }
                if matches!(target.as_ref(), Expr::Name(name) if name.value == "Float")
                    && field.value == "from" =>
            {
                TypeInfo::Float
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
        if then_type.is_known() && else_type.is_known() && then_type != else_type {
            self.diagnostics.push(Diagnostic::error(
                registry::K0401,
                self.diagnostic_span(span),
                format!("if arms have incompatible types `{then_type}` and `{else_type}`"),
            ));
            TypeInfo::Unknown
        } else if then_type == else_type {
            then_type
        } else {
            TypeInfo::Unknown
        }
    }

    fn check_string_interpolations(&mut self, literal: &Spanned<StringLiteral>) {
        for interpolation in &literal.value.interpolations {
            let Some(expr) = parse_interpolation_expr(interpolation.span, &interpolation.value)
            else {
                continue;
            };
            let previous_override = self.diagnostic_span_override.replace(interpolation.span);
            self.infer_expr(&expr);
            self.diagnostic_span_override = previous_override;
        }
    }

    fn define_value(&mut self, name: &Spanned<String>, ty: TypeInfo) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(TypedBinding {
                name: name.value.clone(),
                ty,
            });
        }
    }

    fn value_type(&self, name: &str) -> Option<TypeInfo> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev())
            .find(|binding| binding.name == name)
            .map(|binding| binding.ty.clone())
    }

    fn function_return_type(&self, name: &str) -> Option<TypeInfo> {
        self.functions
            .iter()
            .find(|function| function.name == name)
            .map(|function| function.return_type.clone())
    }

    fn diagnostic_span(&self, fallback: Span) -> Span {
        self.diagnostic_span_override.unwrap_or(fallback)
    }

    fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    fn pop_scope(&mut self) {
        let _ = self.scopes.pop();
    }
}

fn collect_functions(module: &Module) -> Vec<FunctionInfo> {
    let mut functions = Vec::new();
    for item in &module.items {
        if let Item::Function(decl) = item {
            let return_type = decl
                .return_type
                .as_ref()
                .map_or(TypeInfo::Unit, TypeInfo::from_ast);
            functions.push(FunctionInfo {
                name: decl.name.value.clone(),
                return_type,
            });
        }
    }
    functions.sort_by(|left, right| left.name.cmp(&right.name));
    functions
}

fn is_int_float_pair(left: &TypeInfo, right: &TypeInfo) -> bool {
    matches!(
        (left, right),
        (TypeInfo::Int, TypeInfo::Float) | (TypeInfo::Float, TypeInfo::Int)
    )
}

fn parse_interpolation_expr(span: Span, source: &str) -> Option<Expr> {
    let wrapped = format!("fn __keel_interp() {{\n{source}\n}}\n");
    let output = parse(span.source, &wrapped);
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

//! Go source emitter for the M3 backend slice.

use keelc_ast::{
    BinaryOp, Block, EnumDecl, Expr, FieldDecl, FunctionDecl, Item, MatchArm, Module, Pattern,
    Stmt, StringLiteral, StructDecl, StructLiteralField, Type, UnaryOp, VariantDecl,
};
use keelc_parse::parse;
use keelc_span::SourceId;
use std::fmt::{self, Write as _};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendError {
    pub message: String,
}

impl BackendError {
    fn unsupported(feature: impl Into<String>) -> Self {
        Self {
            message: format!("Go backend does not yet support {}", feature.into()),
        }
    }
}

impl fmt::Display for BackendError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for BackendError {}

pub fn emit(module: &Module) -> Result<String, BackendError> {
    Emitter::new(module).emit()
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum TypeInfo {
    Int,
    Float,
    Bool,
    String,
    Char,
    Unit,
    Struct(String),
    Enum(String),
    Option(Box<TypeInfo>),
    Result(Box<TypeInfo>, Box<TypeInfo>),
    Union(Vec<TypeInfo>),
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StructInfo {
    name: String,
    fields: Vec<StructFieldInfo>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StructFieldInfo {
    name: String,
    ty: TypeInfo,
    default: Option<Expr>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct EnumInfo {
    name: String,
    variants: Vec<VariantInfo>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VariantInfo {
    name: String,
    fields: Vec<VariantFieldInfo>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct VariantFieldInfo {
    name: String,
    ty: TypeInfo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct FunctionInfo {
    name: String,
    return_type: TypeInfo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Binding {
    name: String,
    ty: TypeInfo,
}

struct Emitter<'a> {
    module: &'a Module,
    structs: Vec<StructInfo>,
    enums: Vec<EnumInfo>,
    functions: Vec<FunctionInfo>,
    scopes: Vec<Vec<Binding>>,
    current_return_type: TypeInfo,
    output: String,
    indent: usize,
    temp_index: usize,
}

impl<'a> Emitter<'a> {
    fn new(module: &'a Module) -> Self {
        Self {
            module,
            structs: collect_structs(module),
            enums: collect_enums(module),
            functions: collect_functions(module),
            scopes: Vec::new(),
            current_return_type: TypeInfo::Unit,
            output: String::new(),
            indent: 0,
            temp_index: 0,
        }
    }

    fn emit(mut self) -> Result<String, BackendError> {
        self.line("package main")?;
        self.line("")?;
        self.line("import \"fmt\"")?;
        self.line("")?;
        self.emit_runtime()?;

        for item in &self.module.items {
            match item {
                Item::Struct(decl) => self.emit_struct_decl(decl)?,
                Item::Enum(decl) => self.emit_enum_decl(decl)?,
                Item::Function(_) | Item::Use(_) | Item::Test(_) => {}
            }
        }

        for item in &self.module.items {
            if let Item::Function(function) = item {
                self.emit_function(function)?;
                self.line("")?;
            }
        }

        Ok(self.output)
    }

    fn emit_runtime(&mut self) -> Result<(), BackendError> {
        self.line("type KeelEnum struct {")?;
        self.indent += 1;
        self.line("tag string")?;
        self.line("values []any")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func Some(value any) KeelEnum {")?;
        self.indent += 1;
        self.line("return KeelEnum{tag: \"Some\", values: []any{value}}")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("var None = KeelEnum{tag: \"None\"}")?;
        self.line("")?;
        self.line("func Ok(value any) KeelEnum {")?;
        self.indent += 1;
        self.line("return KeelEnum{tag: \"Ok\", values: []any{value}}")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func Err(value any) KeelEnum {")?;
        self.indent += 1;
        self.line("return KeelEnum{tag: \"Err\", values: []any{value}}")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.emit_checked_op("checked_div", "KeelEnum", "/", "return None", "Some")?;
        self.emit_checked_op("checked_rem", "KeelEnum", "%", "return None", "Some")?;
        self.emit_checked_op(
            "keelDiv",
            "int64",
            "/",
            r#"panic("K0204: division by zero")"#,
            "",
        )?;
        self.emit_checked_op(
            "keelRem",
            "int64",
            "%",
            r#"panic("K0204: remainder by zero")"#,
            "",
        )?;
        Ok(())
    }

    fn emit_struct_decl(&mut self, decl: &StructDecl) -> Result<(), BackendError> {
        self.line(&format!("type {} struct {{", decl.name.value))?;
        self.indent += 1;
        for field in &decl.fields {
            self.line(&format!(
                "{} {}",
                field.name.value,
                go_type(&type_from_ast(&field.ty))
            ))?;
        }
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        Ok(())
    }

    fn emit_checked_op(
        &mut self,
        name: &str,
        ret: &str,
        op: &str,
        none_branch: &str,
        ok_prefix: &str,
    ) -> Result<(), BackendError> {
        self.line(&format!("func {name}(left int64, right int64) {ret} {{"))?;
        self.indent += 1;
        self.line(&format!("if right == 0 {{ {none_branch} }}"))?;
        if ok_prefix.is_empty() {
            self.line(&format!("return left {op} right"))?;
        } else {
            self.line(&format!("return {}(left {op} right)", ok_prefix))?;
        }
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        Ok(())
    }

    fn emit_enum_decl(&mut self, decl: &EnumDecl) -> Result<(), BackendError> {
        self.line(&format!("type {} = KeelEnum", decl.name.value))?;
        self.line("")?;
        for variant in &decl.variants {
            self.emit_variant_constructor(variant)?;
            self.line("")?;
        }
        Ok(())
    }

    fn emit_variant_constructor(&mut self, variant: &VariantDecl) -> Result<(), BackendError> {
        if variant.fields.is_empty() {
            self.line(&format!(
                "var {} = KeelEnum{{tag: {:?}}}",
                variant.name.value, variant.name.value
            ))?;
            return Ok(());
        }

        write!(self.output, "func {}(", variant.name.value)?;
        for (index, field) in variant.fields.iter().enumerate() {
            if index > 0 {
                self.output.push_str(", ");
            }
            write!(
                self.output,
                "{} {}",
                field.name.value,
                go_type(&type_from_ast(&field.ty))
            )?;
        }
        self.output.push_str(") KeelEnum {\n");
        self.indent += 1;
        let values = variant
            .fields
            .iter()
            .map(|field| field.name.value.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        self.line(&format!(
            "return KeelEnum{{tag: {:?}, values: []any{{{values}}}}}",
            variant.name.value
        ))?;
        self.indent -= 1;
        self.line("}")?;
        Ok(())
    }

    fn emit_function(&mut self, function: &FunctionDecl) -> Result<(), BackendError> {
        let Some(body) = &function.body else {
            return Err(BackendError::unsupported(
                "function declarations without bodies",
            ));
        };

        write!(self.output, "func {}", function.name.value)?;
        self.output.push('(');
        for (index, param) in function.params.iter().enumerate() {
            if index > 0 {
                self.output.push_str(", ");
            }
            let Some(ty) = &param.ty else {
                return Err(BackendError::unsupported("parameters without types"));
            };
            write!(
                self.output,
                "{} {}",
                param.name.value,
                go_type(&type_from_ast(ty))
            )?;
        }
        self.output.push(')');

        let return_type = function
            .return_type
            .as_ref()
            .map_or(TypeInfo::Unit, type_from_ast);
        if return_type != TypeInfo::Unit {
            self.output.push(' ');
            self.output.push_str(&go_type(&return_type));
        }
        self.output.push_str(" {\n");

        let previous_return_type = std::mem::replace(&mut self.current_return_type, return_type);
        self.push_scope();
        for param in &function.params {
            let ty = param.ty.as_ref().map_or(TypeInfo::Unknown, type_from_ast);
            self.define(&param.name.value, ty);
        }
        self.indent += 1;
        self.emit_block_statements(body, self.current_return_type != TypeInfo::Unit)?;
        self.indent -= 1;
        self.pop_scope();
        self.current_return_type = previous_return_type;

        self.line("}")?;
        Ok(())
    }

    fn emit_block_statements(
        &mut self,
        block: &Block,
        return_last_expr: bool,
    ) -> Result<(), BackendError> {
        self.push_scope();
        for (index, statement) in block.statements.iter().enumerate() {
            let is_last = index + 1 == block.statements.len();
            if return_last_expr && is_last {
                if let Stmt::Expr(expr) = statement {
                    let expr = self.emit_expr(expr)?;
                    self.line(&format!("return {expr}"))?;
                    continue;
                }
            }
            self.emit_stmt(statement)?;
        }
        self.pop_scope();
        Ok(())
    }

    fn emit_stmt(&mut self, statement: &Stmt) -> Result<(), BackendError> {
        match statement {
            Stmt::Let { name, value, .. } => {
                let ty = self.infer_expr(value);
                match value {
                    Expr::Question { expr, .. } => {
                        self.emit_question_let(name.value.as_str(), expr)?
                    }
                    Expr::Catch {
                        expr,
                        error_name,
                        arms,
                        ..
                    } => self.emit_catch_let(
                        name.value.as_str(),
                        expr,
                        error_name.value.as_str(),
                        arms,
                    )?,
                    _ => {
                        let expr = self.emit_expr(value)?;
                        self.line(&format!("{} := {expr}", name.value))?;
                    }
                }
                self.define(&name.value, ty);
                Ok(())
            }
            Stmt::Assign { target, value, .. } => {
                let target = self.emit_expr(target)?;
                let value = self.emit_expr(value)?;
                self.line(&format!("{target} = {value}"))
            }
            Stmt::Return { value, .. } => self.emit_return_stmt(value.as_ref()),
            Stmt::Expr(Expr::If {
                condition,
                then_block,
                else_branch,
                ..
            }) => self.emit_if_stmt(condition, then_block, else_branch.as_deref()),
            Stmt::Expr(Expr::While {
                condition, body, ..
            }) => self.emit_while_stmt(condition, body),
            Stmt::Expr(expr) => {
                let expr = self.emit_expr(expr)?;
                self.line(&expr)
            }
            Stmt::Assert { .. } => Err(BackendError::unsupported("test assertions")),
            Stmt::Break(_) => self.line("break"),
            Stmt::Continue(_) => self.line("continue"),
        }
    }

    fn emit_return_stmt(&mut self, value: Option<&Expr>) -> Result<(), BackendError> {
        if let Some(value) = value {
            let expr = self.emit_expr(value)?;
            self.line(&format!("return {expr}"))
        } else {
            self.line("return")
        }
    }

    fn emit_question_let(&mut self, name: &str, expr: &Expr) -> Result<(), BackendError> {
        let temp = self.next_temp();
        let expr_type = self.infer_expr(expr);
        let success_type = question_success_type(&expr_type).unwrap_or(TypeInfo::Unknown);
        let expr = self.emit_expr(expr)?;
        self.line(&format!("{temp} := {expr}"))?;
        match expr_type {
            TypeInfo::Result(_, _) => {
                self.line(&format!("if {temp}.tag == \"Err\" {{"))?;
                self.indent += 1;
                self.line(&format!("return Err({}.values[0])", temp))?;
                self.indent -= 1;
                self.line("}")?;
                self.line(&format!(
                    "{} := {}",
                    name,
                    payload_expr(&temp, 0, &success_type)
                ))
            }
            TypeInfo::Option(_) => {
                self.line(&format!("if {temp}.tag == \"None\" {{"))?;
                self.indent += 1;
                self.line("return None")?;
                self.indent -= 1;
                self.line("}")?;
                self.line(&format!(
                    "{} := {}",
                    name,
                    payload_expr(&temp, 0, &success_type)
                ))
            }
            _ => Err(BackendError::unsupported("? on non-Result/Option value")),
        }
    }

    fn emit_catch_let(
        &mut self,
        name: &str,
        expr: &Expr,
        error_name: &str,
        arms: &[MatchArm],
    ) -> Result<(), BackendError> {
        let temp = self.next_temp();
        let err_temp = self.next_temp();
        let expr_type = self.infer_expr(expr);
        let (success_type, error_type) = match expr_type {
            TypeInfo::Result(ok, err) => (*ok, *err),
            _ => (TypeInfo::Unknown, TypeInfo::Unknown),
        };
        let expr = self.emit_expr(expr)?;
        self.line(&format!("{temp} := {expr}"))?;
        self.line(&format!("var {} {}", name, go_type(&success_type)))?;
        self.line(&format!("if {temp}.tag == \"Ok\" {{"))?;
        self.indent += 1;
        self.line(&format!(
            "{name} = {}",
            payload_expr(&temp, 0, &success_type)
        ))?;
        self.indent -= 1;
        self.line("} else {")?;
        self.indent += 1;
        self.line(&format!(
            "{} := {}",
            err_temp,
            payload_expr(&temp, 0, &error_type)
        ))?;
        self.define(error_name, error_type.clone());
        self.emit_catch_arms(&err_temp, &error_type, arms)?;
        self.indent -= 1;
        self.line("}")?;
        Ok(())
    }

    fn emit_catch_arms(
        &mut self,
        err_temp: &str,
        error_type: &TypeInfo,
        arms: &[MatchArm],
    ) -> Result<(), BackendError> {
        for arm in arms {
            match &arm.pattern {
                Pattern::Wildcard(_) => self.line("if true {")?,
                Pattern::Name { name, args, .. } if name.value == "other" && args.is_empty() => {
                    self.line("if true {")?;
                    self.indent += 1;
                    self.line(&format!("{} := {err_temp}", name.value))?;
                    self.indent -= 1;
                    self.define(&name.value, error_type.clone());
                }
                Pattern::Name { name, args, .. } => {
                    self.line(&format!("if {err_temp}.tag == {:?} {{", name.value))?;
                    self.indent += 1;
                    let payload_types = self.pattern_payload_types(error_type, &name.value);
                    self.emit_pattern_bindings(err_temp, args, &payload_types)?;
                    self.emit_catch_arm_value(&arm.value)?;
                    self.indent -= 1;
                    self.line("}")?;
                    continue;
                }
            }
            self.indent += 1;
            self.emit_catch_arm_value(&arm.value)?;
            self.indent -= 1;
            self.line("}")?;
        }
        Ok(())
    }

    fn emit_catch_arm_value(&mut self, value: &Expr) -> Result<(), BackendError> {
        match value {
            Expr::Return { value, .. } => {
                let value = value.as_deref();
                self.emit_return_stmt(value)
            }
            _ => {
                let expr = self.emit_expr(value)?;
                self.line(&expr)
            }
        }
    }

    fn emit_expr(&mut self, expr: &Expr) -> Result<String, BackendError> {
        match expr {
            Expr::Int(value) => Ok(value.value.replace('_', "")),
            Expr::Float(value) => Ok(value.value.replace('_', "")),
            Expr::String(value) => self.emit_string(&value.value),
            Expr::Char(value) => Ok(format!("{:?}", value.value)),
            Expr::Bool(value) => Ok(value.value.to_string()),
            Expr::Name(name) => Ok(name.value.clone()),
            Expr::Unary { op, expr, .. } => {
                let expr = self.emit_expr(expr)?;
                let op = match op {
                    UnaryOp::Negate => "-",
                    UnaryOp::Not => "!",
                };
                Ok(format!("({op}{expr})"))
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                let left = self.emit_expr(left)?;
                let right = self.emit_expr(right)?;
                if *op == BinaryOp::Divide {
                    return Ok(format!("keelDiv({left}, {right})"));
                }
                if *op == BinaryOp::Remainder {
                    return Ok(format!("keelRem({left}, {right})"));
                }
                Ok(format!("({left} {} {right})", go_binary_op(*op)))
            }
            Expr::Call { callee, args, .. } => self.emit_call(callee, args),
            Expr::Field { target, field, .. } => {
                let target = self.emit_expr(target)?;
                Ok(format!("{target}.{}", field.value))
            }
            Expr::StructLiteral { name, fields, .. } => {
                self.emit_struct_literal(name.value.as_str(), fields)
            }
            Expr::Block(block) => self.emit_block_expr(block),
            Expr::If {
                condition,
                then_block,
                else_branch,
                ..
            } => self.emit_if_expr(condition, then_block, else_branch.as_deref()),
            Expr::Match {
                scrutinee, arms, ..
            } => self.emit_match_expr(scrutinee, arms),
            Expr::While { .. } => Err(BackendError::unsupported("while expressions")),
            Expr::Question { .. } => Err(BackendError::unsupported(
                "the ? operator outside statement lowering",
            )),
            Expr::Catch { .. } => Err(BackendError::unsupported(
                "catch expressions outside statement lowering",
            )),
            Expr::Return { value, .. } => {
                if let Some(value) = value {
                    let expr = self.emit_expr(value)?;
                    Ok(format!("return {expr}"))
                } else {
                    Ok("return".to_string())
                }
            }
            Expr::Missing(_) | Expr::Wildcard(_) => {
                Err(BackendError::unsupported("missing expressions"))
            }
        }
    }

    fn emit_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<String, BackendError> {
        if matches!(
            callee,
            Expr::Name(name) if name.value == "Some" || name.value == "Ok" || name.value == "Err"
        ) {
            let Some(arg) = args.first() else {
                return Err(BackendError::unsupported("constructor without an argument"));
            };
            let Expr::Name(name) = callee else {
                return Err(BackendError::unsupported("constructor callee"));
            };
            let arg_type = self.infer_expr(arg);
            let arg = cast_constructor_arg(self.emit_expr(arg)?, &arg_type);
            return Ok(format!("{}({arg})", name.value));
        }

        let args = args
            .iter()
            .map(|arg| self.emit_expr(arg))
            .collect::<Result<Vec<_>, _>>()?;
        if matches!(callee, Expr::Name(name) if name.value == "print") {
            return Ok(format!("fmt.Println({})", args.join(", ")));
        }
        if matches!(
            callee,
            Expr::Field { target, field, .. }
                if matches!(target.as_ref(), Expr::Name(name) if name.value == "Float")
                    && field.value == "from"
        ) {
            let Some(arg) = args.first() else {
                return Err(BackendError::unsupported("Float.from without an argument"));
            };
            return Ok(format!("float64({arg})"));
        }
        let callee = self.emit_expr(callee)?;
        Ok(format!("{callee}({})", args.join(", ")))
    }

    fn emit_struct_literal(
        &mut self,
        name: &str,
        fields: &[StructLiteralField],
    ) -> Result<String, BackendError> {
        let Some(info) = self.structs.iter().find(|info| info.name == name).cloned() else {
            return Err(BackendError::unsupported(format!(
                "struct literal `{name}`"
            )));
        };
        let mut parts = Vec::new();
        for field in &info.fields {
            let value = if let Some(provided) = fields
                .iter()
                .find(|provided| provided.name.value == field.name)
            {
                self.emit_expr(&provided.value)?
            } else if let Some(default) = &field.default {
                self.emit_expr(default)?
            } else {
                zero_value(&field.ty).to_string()
            };
            parts.push(format!("{}: {}", field.name, value));
        }
        Ok(format!("{name}{{{}}}", parts.join(", ")))
    }

    fn emit_if_stmt(
        &mut self,
        condition: &Expr,
        then_block: &Block,
        else_branch: Option<&Expr>,
    ) -> Result<(), BackendError> {
        let condition = self.emit_expr(condition)?;
        self.line(&format!("if {condition} {{"))?;
        self.indent += 1;
        self.emit_block_statements(then_block, false)?;
        self.indent -= 1;
        if let Some(else_branch) = else_branch {
            self.line("} else {")?;
            self.indent += 1;
            match else_branch {
                Expr::Block(block) => self.emit_block_statements(block, false)?,
                Expr::If {
                    condition,
                    then_block,
                    else_branch,
                    ..
                } => self.emit_if_stmt(condition, then_block, else_branch.as_deref())?,
                expr => {
                    let expr = self.emit_expr(expr)?;
                    self.line(&expr)?;
                }
            }
            self.indent -= 1;
        }
        self.line("}")?;
        Ok(())
    }

    fn emit_while_stmt(&mut self, condition: &Expr, body: &Block) -> Result<(), BackendError> {
        let condition = self.emit_expr(condition)?;
        self.line(&format!("for {condition} {{"))?;
        self.indent += 1;
        self.emit_block_statements(body, false)?;
        self.indent -= 1;
        self.line("}")?;
        Ok(())
    }

    fn emit_if_expr(
        &mut self,
        condition: &Expr,
        then_block: &Block,
        else_branch: Option<&Expr>,
    ) -> Result<String, BackendError> {
        let Some(else_branch) = else_branch else {
            return Err(BackendError::unsupported("if expressions without else"));
        };
        let condition = self.emit_expr(condition)?;
        let ty = go_type(&self.infer_expr(else_branch));
        let then_body = self.emit_returning_block(then_block)?;
        let else_body = match else_branch {
            Expr::Block(block) => self.emit_returning_block(block)?,
            expr => format!("return {}", self.emit_expr(expr)?),
        };
        Ok(format!(
            "func() {ty} {{ if {condition} {{ {then_body} }} else {{ {else_body} }} }}()"
        ))
    }

    fn emit_string(&mut self, literal: &StringLiteral) -> Result<String, BackendError> {
        if !literal.interpolations.is_empty() {
            let mut args = Vec::new();
            let mut cursor = 0usize;
            for interpolation in &literal.interpolations {
                let needle = format!("{{{}}}", interpolation.value);
                let Some(relative_start) = literal.text[cursor..].find(&needle) else {
                    return Err(BackendError::unsupported(
                        "string interpolation with duplicate or reordered placeholders",
                    ));
                };
                let start = cursor + relative_start;
                if start > cursor {
                    args.push(format!("{:?}", &literal.text[cursor..start]));
                }
                let expr = parse_interpolation_expr(&interpolation.value)?;
                args.push(self.emit_expr(&expr)?);
                cursor = start + needle.len();
            }
            if cursor < literal.text.len() {
                args.push(format!("{:?}", &literal.text[cursor..]));
            }
            if args.is_empty() {
                return Ok("\"\"".to_string());
            }
            return Ok(format!("fmt.Sprint({})", args.join(", ")));
        }
        Ok(format!("{:?}", literal.text))
    }

    fn emit_block_expr(&mut self, block: &Block) -> Result<String, BackendError> {
        let ty = go_type(&self.infer_block_type(block));
        let body = self.emit_returning_block(block)?;
        Ok(format!("func() {ty} {{ {body} }}()"))
    }

    fn emit_returning_block(&mut self, block: &Block) -> Result<String, BackendError> {
        let Some((last, prefix)) = block.statements.split_last() else {
            return Err(BackendError::unsupported("empty block expressions"));
        };
        self.push_scope();
        let mut output = String::new();
        for statement in prefix {
            output.push_str(&self.emit_inline_stmt(statement)?);
            output.push(' ');
        }
        match last {
            Stmt::Expr(expr) => {
                output.push_str("return ");
                output.push_str(&self.emit_expr(expr)?);
            }
            Stmt::Return { value, .. } => {
                output.push_str("return");
                if let Some(value) = value {
                    output.push(' ');
                    output.push_str(&self.emit_expr(value)?);
                }
            }
            _ => return Err(BackendError::unsupported("non-expression block values")),
        }
        self.pop_scope();
        Ok(output)
    }

    fn emit_inline_stmt(&mut self, statement: &Stmt) -> Result<String, BackendError> {
        match statement {
            Stmt::Let { name, value, .. } => {
                let ty = self.infer_expr(value);
                let expr = self.emit_expr(value)?;
                self.define(&name.value, ty);
                Ok(format!("{} := {};", name.value, expr))
            }
            Stmt::Assign { target, value, .. } => Ok(format!(
                "{} = {};",
                self.emit_expr(target)?,
                self.emit_expr(value)?
            )),
            Stmt::Expr(expr) => Ok(format!("{};", self.emit_expr(expr)?)),
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    Ok(format!("return {};", self.emit_expr(value)?))
                } else {
                    Ok("return;".to_string())
                }
            }
            Stmt::Assert { .. } => Err(BackendError::unsupported("test assertions")),
            Stmt::Break(_) | Stmt::Continue(_) => Err(BackendError::unsupported(
                "break/continue in block expressions",
            )),
        }
    }

    fn emit_match_expr(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchArm],
    ) -> Result<String, BackendError> {
        let scrutinee_ty = self.infer_expr(scrutinee);
        let result_ty = infer_match_result(self, arms);
        let returns_value = result_ty != TypeInfo::Unit;
        let result_go_type = go_type(&result_ty);
        let temp = self.next_temp();
        let scrutinee_expr = self.emit_expr(scrutinee)?;
        let mut out = String::new();
        if returns_value {
            write!(out, "func() {result_go_type} {{ ")?;
        } else {
            out.push_str("func() { ");
        }
        write!(out, "{temp} := {scrutinee_expr}; ")?;
        for arm in arms {
            self.write_match_arm(&mut out, &temp, &scrutinee_ty, &result_ty, arm)?;
        }
        if returns_value {
            write!(out, "return {}; ", zero_value(&result_ty))?;
        }
        out.push_str("}()");
        Ok(out)
    }

    fn write_match_arm(
        &mut self,
        out: &mut String,
        temp: &str,
        scrutinee_ty: &TypeInfo,
        result_ty: &TypeInfo,
        arm: &MatchArm,
    ) -> Result<(), BackendError> {
        match &arm.pattern {
            Pattern::Wildcard(_) => out.push_str("if true { "),
            Pattern::Name { name, args, .. } => {
                write!(out, "if {temp}.tag == {:?} {{ ", name.value)?;
                self.push_scope();
                let payload_types = self.pattern_payload_types(scrutinee_ty, &name.value);
                let bindings = self.inline_pattern_bindings(temp, args, &payload_types)?;
                out.push_str(&bindings);
                if let Some(guard) = &arm.guard {
                    let guard = self.emit_expr(guard)?;
                    write!(out, "if {guard} {{ ")?;
                    self.write_match_arm_value(out, result_ty, &arm.value)?;
                    out.push_str("}; ");
                } else {
                    self.write_match_arm_value(out, result_ty, &arm.value)?;
                }
                self.pop_scope();
                out.push_str("}; ");
                return Ok(());
            }
        }

        self.push_scope();
        self.write_match_arm_value(out, result_ty, &arm.value)?;
        self.pop_scope();
        out.push_str("}; ");
        Ok(())
    }

    fn write_match_arm_value(
        &mut self,
        out: &mut String,
        result_ty: &TypeInfo,
        value: &Expr,
    ) -> Result<(), BackendError> {
        if *result_ty == TypeInfo::Unit {
            let expr = self.emit_expr(value)?;
            write!(out, "{expr}; return ")?;
        } else {
            let expr = self.emit_expr(value)?;
            write!(out, "return {expr} ")?;
        }
        Ok(())
    }

    fn emit_pattern_bindings(
        &mut self,
        temp: &str,
        args: &[Pattern],
        payload_types: &[TypeInfo],
    ) -> Result<(), BackendError> {
        self.emit_pattern_bindings_inner(temp, args, payload_types, false)?;
        Ok(())
    }

    fn inline_pattern_bindings(
        &mut self,
        temp: &str,
        args: &[Pattern],
        payload_types: &[TypeInfo],
    ) -> Result<String, BackendError> {
        self.emit_pattern_bindings_inner(temp, args, payload_types, true)
    }

    fn emit_pattern_bindings_inner(
        &mut self,
        temp: &str,
        args: &[Pattern],
        payload_types: &[TypeInfo],
        inline: bool,
    ) -> Result<String, BackendError> {
        let mut out = String::new();
        for (index, pattern) in args.iter().enumerate() {
            let ty = payload_types
                .get(index)
                .cloned()
                .unwrap_or(TypeInfo::Unknown);
            if let Pattern::Name { name, args, .. } = pattern {
                if !args.is_empty() {
                    return Err(BackendError::unsupported("nested pattern bindings"));
                }
                if inline {
                    write!(
                        out,
                        "{} := {}; _ = {}; ",
                        name.value,
                        payload_expr(temp, index, &ty),
                        name.value
                    )?;
                } else {
                    self.line(&format!(
                        "{} := {}",
                        name.value,
                        payload_expr(temp, index, &ty)
                    ))?;
                    self.line(&format!("_ = {}", name.value))?;
                }
                self.define(&name.value, ty);
            }
        }
        Ok(out)
    }

    fn pattern_payload_types(&self, scrutinee_ty: &TypeInfo, pattern_name: &str) -> Vec<TypeInfo> {
        match scrutinee_ty {
            TypeInfo::Option(inner) if pattern_name == "Some" => vec![inner.as_ref().clone()],
            TypeInfo::Result(ok, _) if pattern_name == "Ok" => vec![ok.as_ref().clone()],
            TypeInfo::Result(_, err) if pattern_name == "Err" => vec![err.as_ref().clone()],
            TypeInfo::Enum(enum_name) | TypeInfo::Struct(enum_name) => self
                .enums
                .iter()
                .find(|info| info.name == *enum_name)
                .and_then(|info| {
                    info.variants
                        .iter()
                        .find(|variant| variant.name == pattern_name)
                })
                .map(|variant| {
                    variant
                        .fields
                        .iter()
                        .map(|field| field.ty.clone())
                        .collect()
                })
                .unwrap_or_default(),
            _ => Vec::new(),
        }
    }

    fn infer_block_type(&self, block: &Block) -> TypeInfo {
        block
            .statements
            .last()
            .map_or(TypeInfo::Unit, |statement| match statement {
                Stmt::Expr(expr) => self.infer_expr(expr),
                Stmt::Return {
                    value: Some(expr), ..
                } => self.infer_expr(expr),
                _ => TypeInfo::Unit,
            })
    }

    fn infer_expr(&self, expr: &Expr) -> TypeInfo {
        match expr {
            Expr::Missing(_) | Expr::Wildcard(_) => TypeInfo::Unknown,
            Expr::Int(_) => TypeInfo::Int,
            Expr::Float(_) => TypeInfo::Float,
            Expr::String(_) => TypeInfo::String,
            Expr::Char(_) => TypeInfo::Char,
            Expr::Bool(_) => TypeInfo::Bool,
            Expr::Name(name) => self
                .value_type(&name.value)
                .or_else(|| self.builtin_value_type(&name.value))
                .or_else(|| self.enum_variant_type(&name.value))
                .unwrap_or(TypeInfo::Unknown),
            Expr::Unary { op, expr, .. } => match op {
                UnaryOp::Negate => self.infer_expr(expr),
                UnaryOp::Not => TypeInfo::Bool,
            },
            Expr::Binary { left, op, .. } => match op {
                BinaryOp::Add
                | BinaryOp::Subtract
                | BinaryOp::Multiply
                | BinaryOp::Divide
                | BinaryOp::Remainder => self.infer_expr(left),
                BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual
                | BinaryOp::And
                | BinaryOp::Or => TypeInfo::Bool,
            },
            Expr::Call { callee, args, .. } => self.infer_call(callee, args),
            Expr::Field { target, field, .. } => {
                let target_ty = self.infer_expr(target);
                self.field_type(&target_ty, &field.value)
                    .unwrap_or(TypeInfo::Unknown)
            }
            Expr::StructLiteral { name, .. } => TypeInfo::Struct(name.value.clone()),
            Expr::If {
                then_block,
                else_branch,
                ..
            } => else_branch
                .as_deref()
                .map_or(TypeInfo::Unit, |else_branch| {
                    merge_types(
                        &self.infer_block_type(then_block),
                        &self.infer_expr(else_branch),
                    )
                }),
            Expr::Match { arms, .. } => infer_match_result(self, arms),
            Expr::While { .. } => TypeInfo::Unit,
            Expr::Block(block) => self.infer_block_type(block),
            Expr::Question { expr, .. } => {
                question_success_type(&self.infer_expr(expr)).unwrap_or(TypeInfo::Unknown)
            }
            Expr::Catch { expr, .. } => match self.infer_expr(expr) {
                TypeInfo::Result(ok, _) => *ok,
                _ => TypeInfo::Unknown,
            },
            Expr::Return { .. } => TypeInfo::Unit,
        }
    }

    fn infer_call(&self, callee: &Expr, args: &[Expr]) -> TypeInfo {
        let arg_types: Vec<TypeInfo> = args.iter().map(|arg| self.infer_expr(arg)).collect();
        match callee {
            Expr::Name(name) if name.value == "print" => TypeInfo::Unit,
            Expr::Name(name) if name.value == "checked_div" || name.value == "checked_rem" => {
                TypeInfo::Option(Box::new(TypeInfo::Int))
            }
            Expr::Name(name) if name.value == "Some" => TypeInfo::Option(Box::new(
                arg_types.first().cloned().unwrap_or(TypeInfo::Unknown),
            )),
            Expr::Name(name) if name.value == "Ok" => TypeInfo::Result(
                Box::new(arg_types.first().cloned().unwrap_or(TypeInfo::Unknown)),
                Box::new(TypeInfo::Unknown),
            ),
            Expr::Name(name) if name.value == "Err" => TypeInfo::Result(
                Box::new(TypeInfo::Unknown),
                Box::new(arg_types.first().cloned().unwrap_or(TypeInfo::Unknown)),
            ),
            Expr::Name(name) => self
                .function_return_type(&name.value)
                .or_else(|| self.enum_variant_type(&name.value))
                .unwrap_or(TypeInfo::Unknown),
            Expr::Field { target, field, .. }
                if matches!(target.as_ref(), Expr::Name(name) if name.value == "Float")
                    && field.value == "from" =>
            {
                TypeInfo::Float
            }
            _ => TypeInfo::Unknown,
        }
    }

    fn field_type(&self, target_ty: &TypeInfo, field_name: &str) -> Option<TypeInfo> {
        let TypeInfo::Struct(name) = target_ty else {
            return None;
        };
        self.structs
            .iter()
            .find(|info| info.name == *name)?
            .fields
            .iter()
            .find(|field| field.name == field_name)
            .map(|field| field.ty.clone())
    }

    fn builtin_value_type(&self, name: &str) -> Option<TypeInfo> {
        match name {
            "None" => Some(TypeInfo::Option(Box::new(TypeInfo::Unknown))),
            _ => None,
        }
    }

    fn enum_variant_type(&self, variant_name: &str) -> Option<TypeInfo> {
        self.enums
            .iter()
            .find(|info| {
                info.variants
                    .iter()
                    .any(|variant| variant.name == variant_name)
            })
            .map(|info| TypeInfo::Enum(info.name.clone()))
    }

    fn function_return_type(&self, name: &str) -> Option<TypeInfo> {
        self.functions
            .iter()
            .find(|function| function.name == name)
            .map(|function| function.return_type.clone())
    }

    fn define(&mut self, name: &str, ty: TypeInfo) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(Binding {
                name: name.to_string(),
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

    fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    fn pop_scope(&mut self) {
        let _ = self.scopes.pop();
    }

    fn next_temp(&mut self) -> String {
        let temp = format!("__keel_tmp_{}", self.temp_index);
        self.temp_index += 1;
        temp
    }

    fn line(&mut self, text: &str) -> Result<(), BackendError> {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        writeln!(self.output, "{text}")?;
        Ok(())
    }
}

fn infer_match_result(emitter: &Emitter<'_>, arms: &[MatchArm]) -> TypeInfo {
    let mut result = TypeInfo::Unknown;
    for arm in arms {
        let arm_type = emitter.infer_expr(&arm.value);
        result = merge_types(&result, &arm_type);
    }
    result
}

fn parse_interpolation_expr(source: &str) -> Result<Expr, BackendError> {
    let wrapped = format!("fn __keel_interp() {{\n{source}\n}}\n");
    let output = parse(SourceId::new(0), &wrapped);
    if !output.diagnostics.is_empty() {
        return Err(BackendError::unsupported("malformed string interpolation"));
    }

    output
        .module
        .items
        .iter()
        .find_map(|item| {
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
        .ok_or_else(|| BackendError::unsupported("empty string interpolation"))
}

fn collect_structs(module: &Module) -> Vec<StructInfo> {
    let mut structs = Vec::new();
    for item in &module.items {
        if let Item::Struct(decl) = item {
            structs.push(StructInfo {
                name: decl.name.value.clone(),
                fields: decl.fields.iter().map(struct_field_info).collect(),
            });
        }
    }
    structs.sort_by(|left, right| left.name.cmp(&right.name));
    structs
}

fn struct_field_info(field: &FieldDecl) -> StructFieldInfo {
    StructFieldInfo {
        name: field.name.value.clone(),
        ty: type_from_ast(&field.ty),
        default: field.default.clone(),
    }
}

fn collect_enums(module: &Module) -> Vec<EnumInfo> {
    let mut enums = Vec::new();
    for item in &module.items {
        if let Item::Enum(decl) = item {
            enums.push(EnumInfo {
                name: decl.name.value.clone(),
                variants: decl.variants.iter().map(variant_info).collect(),
            });
        }
    }
    enums.sort_by(|left, right| left.name.cmp(&right.name));
    enums
}

fn variant_info(variant: &VariantDecl) -> VariantInfo {
    VariantInfo {
        name: variant.name.value.clone(),
        fields: variant
            .fields
            .iter()
            .map(|field| VariantFieldInfo {
                name: field.name.value.clone(),
                ty: type_from_ast(&field.ty),
            })
            .collect(),
    }
}

fn collect_functions(module: &Module) -> Vec<FunctionInfo> {
    let mut functions = Vec::new();
    for item in &module.items {
        if let Item::Function(decl) = item {
            functions.push(FunctionInfo {
                name: decl.name.value.clone(),
                return_type: decl
                    .return_type
                    .as_ref()
                    .map_or(TypeInfo::Unit, type_from_ast),
            });
        }
    }
    functions.sort_by(|left, right| left.name.cmp(&right.name));
    functions
}

fn type_from_ast(ty: &Type) -> TypeInfo {
    match ty {
        Type::Named { name, args, .. } if args.is_empty() => match name.value.as_str() {
            "Int" => TypeInfo::Int,
            "Float" => TypeInfo::Float,
            "Bool" => TypeInfo::Bool,
            "String" => TypeInfo::String,
            "Char" => TypeInfo::Char,
            "Unit" => TypeInfo::Unit,
            other => TypeInfo::Struct(other.to_string()),
        },
        Type::Named { name, args, .. } if name.value == "Option" && args.len() == 1 => {
            TypeInfo::Option(Box::new(type_from_ast(&args[0])))
        }
        Type::Named { name, args, .. } if name.value == "Result" && args.len() == 2 => {
            TypeInfo::Result(
                Box::new(type_from_ast(&args[0])),
                Box::new(type_from_ast(&args[1])),
            )
        }
        Type::Named { name, .. } => TypeInfo::Struct(name.value.clone()),
        Type::Union { members, .. } => TypeInfo::Union(members.iter().map(type_from_ast).collect()),
    }
}

fn go_type(ty: &TypeInfo) -> String {
    match ty {
        TypeInfo::Int => "int64".to_string(),
        TypeInfo::Float => "float64".to_string(),
        TypeInfo::Bool => "bool".to_string(),
        TypeInfo::String => "string".to_string(),
        TypeInfo::Char => "rune".to_string(),
        TypeInfo::Unit => String::new(),
        TypeInfo::Struct(name) => name.clone(),
        TypeInfo::Enum(_) | TypeInfo::Option(_) | TypeInfo::Result(_, _) | TypeInfo::Union(_) => {
            "KeelEnum".to_string()
        }
        TypeInfo::Unknown => "any".to_string(),
    }
}

fn merge_types(left: &TypeInfo, right: &TypeInfo) -> TypeInfo {
    if matches!(left, TypeInfo::Unknown) {
        return right.clone();
    }
    if matches!(right, TypeInfo::Unknown) || left == right {
        return left.clone();
    }
    match (left, right) {
        (TypeInfo::Option(left), TypeInfo::Option(right)) => {
            TypeInfo::Option(Box::new(merge_types(left, right)))
        }
        (TypeInfo::Result(left_ok, left_err), TypeInfo::Result(right_ok, right_err)) => {
            TypeInfo::Result(
                Box::new(merge_types(left_ok, right_ok)),
                Box::new(merge_types(left_err, right_err)),
            )
        }
        _ => TypeInfo::Unknown,
    }
}

fn question_success_type(ty: &TypeInfo) -> Option<TypeInfo> {
    match ty {
        TypeInfo::Option(inner) => Some(inner.as_ref().clone()),
        TypeInfo::Result(ok, _) => Some(ok.as_ref().clone()),
        _ => None,
    }
}

fn payload_expr(value: &str, index: usize, ty: &TypeInfo) -> String {
    let raw = format!("{value}.values[{index}]");
    match ty {
        TypeInfo::Unknown => raw,
        _ => format!("{raw}.({})", go_type(ty)),
    }
}

fn cast_constructor_arg(expr: String, ty: &TypeInfo) -> String {
    match ty {
        TypeInfo::Int => format!("int64({expr})"),
        TypeInfo::Float => format!("float64({expr})"),
        TypeInfo::Char => format!("rune({expr})"),
        _ => expr,
    }
}

fn zero_value(ty: &TypeInfo) -> &'static str {
    match ty {
        TypeInfo::Int | TypeInfo::Float | TypeInfo::Char => "0",
        TypeInfo::Bool => "false",
        TypeInfo::String => "\"\"",
        TypeInfo::Unit => "",
        TypeInfo::Struct(_)
        | TypeInfo::Enum(_)
        | TypeInfo::Option(_)
        | TypeInfo::Result(_, _)
        | TypeInfo::Union(_) => "KeelEnum{}",
        TypeInfo::Unknown => "nil",
    }
}

fn go_binary_op(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Subtract => "-",
        BinaryOp::Multiply => "*",
        BinaryOp::Divide => "/",
        BinaryOp::Remainder => "%",
        BinaryOp::Equal => "==",
        BinaryOp::NotEqual => "!=",
        BinaryOp::Less => "<",
        BinaryOp::LessEqual => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEqual => ">=",
        BinaryOp::And => "&&",
        BinaryOp::Or => "||",
    }
}

impl From<fmt::Error> for BackendError {
    fn from(_: fmt::Error) -> Self {
        Self {
            message: "failed to write generated Go source".to_string(),
        }
    }
}

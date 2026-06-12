//! Go source emitter for the first M3 backend slice.

use keelc_ast::{
    BinaryOp, Block, Expr, FunctionDecl, Item, Module, Stmt, StringLiteral, Type, UnaryOp,
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

struct Emitter<'a> {
    module: &'a Module,
    output: String,
    indent: usize,
}

impl<'a> Emitter<'a> {
    fn new(module: &'a Module) -> Self {
        Self {
            module,
            output: String::new(),
            indent: 0,
        }
    }

    fn emit(mut self) -> Result<String, BackendError> {
        self.line("package main")?;
        self.line("")?;
        self.line("import \"fmt\"")?;
        self.line("")?;
        self.line("func keelDiv(left int64, right int64) int64 {")?;
        self.indent += 1;
        self.line("if right == 0 { panic(\"K0204: division by zero\") }")?;
        self.line("return left / right")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func keelRem(left int64, right int64) int64 {")?;
        self.indent += 1;
        self.line("if right == 0 { panic(\"K0204: remainder by zero\") }")?;
        self.line("return left % right")?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;

        for item in &self.module.items {
            if let Item::Function(function) = item {
                self.emit_function(function)?;
                self.line("")?;
            }
        }

        Ok(self.output)
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
            write!(self.output, "{} ", param.name.value)?;
            let Some(ty) = &param.ty else {
                return Err(BackendError::unsupported("parameters without types"));
            };
            self.output.push_str(&go_type(ty)?);
        }
        self.output.push(')');
        if let Some(return_type) = &function.return_type {
            if !is_unit_type(return_type) {
                self.output.push(' ');
                self.output.push_str(&go_type(return_type)?);
            }
        }
        self.output.push_str(" {\n");
        self.indent += 1;
        let returns_value = function
            .return_type
            .as_ref()
            .is_some_and(|return_type| !is_unit_type(return_type));
        self.emit_block_statements(body, returns_value)?;
        self.indent -= 1;
        self.line("}")?;
        Ok(())
    }

    fn emit_block_statements(
        &mut self,
        block: &Block,
        return_last_expr: bool,
    ) -> Result<(), BackendError> {
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
        Ok(())
    }

    fn emit_stmt(&mut self, statement: &Stmt) -> Result<(), BackendError> {
        match statement {
            Stmt::Let { name, value, .. } => {
                let expr = self.emit_expr(value)?;
                self.line(&format!("{} := {expr}", name.value))
            }
            Stmt::Assign { target, value, .. } => {
                let target = self.emit_expr(target)?;
                let value = self.emit_expr(value)?;
                self.line(&format!("{target} = {value}"))
            }
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    let expr = self.emit_expr(value)?;
                    self.line(&format!("return {expr}"))
                } else {
                    self.line("return")
                }
            }
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
            Expr::Block(block) => self.emit_block_expr(block),
            Expr::If {
                condition,
                then_block,
                else_branch,
                ..
            } => self.emit_if_expr(condition, then_block, else_branch.as_deref()),
            Expr::Match { .. } => Err(BackendError::unsupported("match expressions")),
            Expr::While { .. } => Err(BackendError::unsupported("while expressions")),
            Expr::StructLiteral { .. } => Err(BackendError::unsupported("struct literals")),
            Expr::Question { .. } => Err(BackendError::unsupported("the ? operator")),
            Expr::Catch { .. } => Err(BackendError::unsupported("catch expressions")),
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
        let ty = self.infer_expr_go_type(else_branch)?;
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
        let ty = self.infer_block_go_type(block)?;
        let body = self.emit_returning_block(block)?;
        Ok(format!("func() {ty} {{ {body} }}()"))
    }

    fn emit_returning_block(&mut self, block: &Block) -> Result<String, BackendError> {
        let Some((last, prefix)) = block.statements.split_last() else {
            return Err(BackendError::unsupported("empty block expressions"));
        };
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
        Ok(output)
    }

    fn emit_inline_stmt(&mut self, statement: &Stmt) -> Result<String, BackendError> {
        match statement {
            Stmt::Let { name, value, .. } => {
                Ok(format!("{} := {};", name.value, self.emit_expr(value)?))
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

    fn infer_block_go_type(&self, block: &Block) -> Result<String, BackendError> {
        let Some(statement) = block.statements.last() else {
            return Err(BackendError::unsupported("empty block expressions"));
        };
        match statement {
            Stmt::Expr(expr) => self.infer_expr_go_type(expr),
            Stmt::Return {
                value: Some(expr), ..
            } => self.infer_expr_go_type(expr),
            _ => Err(BackendError::unsupported("non-expression block values")),
        }
    }

    fn infer_expr_go_type(&self, expr: &Expr) -> Result<String, BackendError> {
        match expr {
            Expr::Int(_) => Ok("int64".to_string()),
            Expr::Float(_) => Ok("float64".to_string()),
            Expr::String(_) => Ok("string".to_string()),
            Expr::Char(_) => Ok("rune".to_string()),
            Expr::Bool(_) => Ok("bool".to_string()),
            Expr::Unary { expr, op, .. } => match op {
                UnaryOp::Negate => self.infer_expr_go_type(expr),
                UnaryOp::Not => Ok("bool".to_string()),
            },
            Expr::Binary { left, op, .. } => match op {
                BinaryOp::Add
                | BinaryOp::Subtract
                | BinaryOp::Multiply
                | BinaryOp::Divide
                | BinaryOp::Remainder => self.infer_expr_go_type(left),
                BinaryOp::Equal
                | BinaryOp::NotEqual
                | BinaryOp::Less
                | BinaryOp::LessEqual
                | BinaryOp::Greater
                | BinaryOp::GreaterEqual
                | BinaryOp::And
                | BinaryOp::Or => Ok("bool".to_string()),
            },
            Expr::Block(block) => self.infer_block_go_type(block),
            Expr::If { else_branch, .. } => {
                let Some(else_branch) = else_branch else {
                    return Err(BackendError::unsupported("if expressions without else"));
                };
                self.infer_expr_go_type(else_branch)
            }
            Expr::Call { callee, .. } => {
                if matches!(callee.as_ref(), Expr::Name(name) if name.value == "print") {
                    Ok(String::new())
                } else if let Expr::Name(name) = callee.as_ref() {
                    self.module
                        .items
                        .iter()
                        .find_map(|item| {
                            let Item::Function(function) = item else {
                                return None;
                            };
                            (function.name.value == name.value)
                                .then_some(function.return_type.as_ref())
                                .flatten()
                        })
                        .map(go_type)
                        .transpose()?
                        .ok_or_else(|| BackendError::unsupported(format!("call `{}`", name.value)))
                } else {
                    Err(BackendError::unsupported("call expressions"))
                }
            }
            Expr::Name(_) => Ok("any".to_string()),
            _ => Err(BackendError::unsupported("type inference for expression")),
        }
    }

    fn line(&mut self, text: &str) -> Result<(), BackendError> {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        writeln!(self.output, "{text}")?;
        Ok(())
    }
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

fn go_type(ty: &Type) -> Result<String, BackendError> {
    match ty {
        Type::Named { name, args, .. } if args.is_empty() => match name.value.as_str() {
            "Int" => Ok("int64".to_string()),
            "Float" => Ok("float64".to_string()),
            "Bool" => Ok("bool".to_string()),
            "String" => Ok("string".to_string()),
            "Char" => Ok("rune".to_string()),
            "Unit" => Ok(String::new()),
            other => Err(BackendError::unsupported(format!("type `{other}`"))),
        },
        Type::Named { name, .. } => Err(BackendError::unsupported(format!(
            "generic type `{}`",
            name.value
        ))),
        Type::Union { .. } => Err(BackendError::unsupported("union types")),
    }
}

fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Named { name, args, .. } if name.value == "Unit" && args.is_empty())
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

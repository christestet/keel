//! AST pretty-printer — the canonical formatter for Keel Core.

use crate::{
    BinaryOp, Block, Expr, FieldDecl, FunctionDecl, ImplDecl, InterfaceDecl, Item, MatchArm,
    Module, Param, Pattern, Stmt, StructLiteralField, TestDecl, Type, TypeParam, UnaryOp,
    VariantDecl,
};

/// Render a module to its canonical Keel Core source form.
#[must_use]
pub fn pretty_print(module: &Module) -> String {
    let mut printer = Printer::new();
    printer.module(module);
    printer.finish()
}

struct Printer {
    output: String,
    indent: usize,
}

impl Printer {
    fn new() -> Self {
        Self {
            output: String::new(),
            indent: 0,
        }
    }

    fn finish(self) -> String {
        self.output
    }

    fn module(&mut self, module: &Module) {
        if let Some(header) = &module.header {
            self.line(&format!("module {}", header.value));
            self.blank_line();
        }
        for (index, item) in module.items.iter().enumerate() {
            if index > 0 {
                self.blank_line();
            }
            self.item(item);
        }
    }

    fn item(&mut self, item: &Item) {
        match item {
            Item::Use(decl) => {
                let path = decl
                    .path
                    .iter()
                    .map(|segment| segment.value.as_str())
                    .collect::<Vec<_>>()
                    .join(".");
                self.line(&format!("use {path}"));
            }
            Item::Struct(decl) => {
                let mut header = format!("struct {}", decl.name.value);
                header.push_str(&self.type_params(&decl.type_params));
                self.line(&format!("{header} {{"));
                self.indent();
                for field in &decl.fields {
                    self.field_decl(field);
                }
                self.dedent();
                self.line("}");
            }
            Item::Enum(decl) => {
                let type_params = self.type_params(&decl.type_params);
                self.line(&format!("enum {type_params}{} {{", decl.name.value));
                self.indent();
                for variant in &decl.variants {
                    self.variant_decl(variant);
                }
                self.dedent();
                self.line("}");
            }
            Item::Function(decl) => self.function_decl(decl),
            Item::Interface(decl) => self.interface_decl(decl),
            Item::Impl(decl) => self.impl_decl(decl),
            Item::Test(decl) => self.test_decl(decl),
        }
    }

    fn field_decl(&mut self, field: &FieldDecl) {
        let mut line = format!("{}: {}", field.name.value, self.type_(&field.ty));
        if let Some(default) = &field.default {
            line.push_str(&format!(" = {}", self.expr(default, 0, self.indent)));
        }
        self.line(&line);
    }

    fn variant_decl(&mut self, variant: &VariantDecl) {
        let mut line = variant.name.value.clone();
        if !variant.fields.is_empty() {
            let fields = variant
                .fields
                .iter()
                .map(|field| format!("{}: {}", field.name.value, self.type_(&field.ty)))
                .collect::<Vec<_>>()
                .join(", ");
            line.push_str(&format!("({fields})"));
        }
        self.line(&line);
    }

    fn function_decl(&mut self, decl: &FunctionDecl) {
        let params = decl
            .params
            .iter()
            .map(|param| self.param(param))
            .collect::<Vec<_>>()
            .join(", ");
        let type_params = self.type_params(&decl.type_params);
        let mut signature = format!("fn {}{type_params}({params})", decl.name.value);
        if let Some(return_type) = &decl.return_type {
            let ty = self.type_(return_type);
            if ty != "Unit" {
                signature.push_str(&format!(" -> {ty}"));
            }
        }
        match &decl.body {
            Some(body) => {
                self.line(&format!("{signature} {{"));
                self.indent();
                for statement in &body.statements {
                    self.stmt(statement);
                }
                self.dedent();
                self.line("}");
            }
            None => self.line(&signature),
        }
    }

    fn interface_decl(&mut self, decl: &InterfaceDecl) {
        self.line(&format!("interface {} {{", decl.name.value));
        self.indent();
        for method in &decl.methods {
            self.function_decl(method);
        }
        self.dedent();
        self.line("}");
    }

    fn impl_decl(&mut self, decl: &ImplDecl) {
        let type_args = if decl.type_args.is_empty() {
            String::new()
        } else {
            let inner = decl
                .type_args
                .iter()
                .map(|arg| self.type_(arg))
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{inner}]")
        };
        self.line(&format!(
            "impl {} for {type_args}{} {{",
            decl.interface_name.value, decl.type_name.value
        ));
        self.indent();
        for method in &decl.methods {
            self.function_decl(method);
        }
        self.dedent();
        self.line("}");
    }

    fn test_decl(&mut self, decl: &TestDecl) {
        self.write(&format!("test \"{}\" ", decl.name.value));
        self.block(&decl.body);
    }

    fn type_params(&self, params: &[TypeParam]) -> String {
        if params.is_empty() {
            return String::new();
        }
        let inner = params
            .iter()
            .map(|param| {
                let name = &param.name.value;
                match &param.bound {
                    Some(bound) => format!("{name}: {}", bound.value),
                    None => name.clone(),
                }
            })
            .collect::<Vec<_>>()
            .join(", ");
        format!("[{inner}]")
    }

    fn param(&self, param: &Param) -> String {
        let name = param.name.value.as_str();
        match &param.ty {
            Some(ty) => format!("{name}: {}", self.type_(ty)),
            None => name.to_string(),
        }
    }

    fn type_(&self, ty: &Type) -> String {
        match ty {
            Type::Named { name, args, .. } => {
                let mut result = name.value.clone();
                if !args.is_empty() {
                    let args = args
                        .iter()
                        .map(|arg| self.type_(arg))
                        .collect::<Vec<_>>()
                        .join(", ");
                    result.push_str(&format!("<{args}>"));
                }
                result
            }
            Type::Union { members, .. } => members
                .iter()
                .map(|member| self.type_(member))
                .collect::<Vec<_>>()
                .join(" | "),
        }
    }

    fn block(&mut self, block: &Block) {
        self.line("{");
        self.indent();
        for statement in &block.statements {
            self.stmt(statement);
        }
        self.dedent();
        self.line("}");
    }

    fn stmt(&mut self, statement: &Stmt) {
        let base = self.indent;
        match statement {
            Stmt::Let {
                mutable,
                name,
                ty,
                value,
                ..
            } => {
                let keyword = if *mutable { "mut" } else { "let" };
                let type_annotation = ty
                    .as_ref()
                    .map(|ty| format!(": {}", self.type_(ty)))
                    .unwrap_or_default();
                self.line(&format!(
                    "{keyword} {}{type_annotation} = {}",
                    name.value,
                    self.expr(value, 0, base)
                ));
            }
            Stmt::Assign { target, value, .. } => {
                self.line(&format!(
                    "{} = {}",
                    self.expr(target, 0, base),
                    self.expr(value, 0, base)
                ));
            }
            Stmt::Return {
                value: Some(value), ..
            } => {
                self.line(&format!("return {}", self.expr(value, 0, base)));
            }
            Stmt::Return { value: None, .. } => self.line("return"),
            Stmt::Break(_) => self.line("break"),
            Stmt::Continue(_) => self.line("continue"),
            Stmt::Assert { value, .. } => {
                self.line(&format!("assert {}", self.expr(value, 0, base)))
            }
            Stmt::Expr(expr) => self.line(&self.expr(expr, 0, base)),
        }
    }

    fn expr(&self, expr: &Expr, parent_prec: u8, base_indent: usize) -> String {
        let (text, prec) = self.expr_inner(expr, base_indent);
        if needs_parens(prec, parent_prec) {
            format!("({text})")
        } else {
            text
        }
    }

    fn expr_inner(&self, expr: &Expr, base_indent: usize) -> (String, u8) {
        match expr {
            Expr::Missing(_) => ("<missing>".to_string(), 0),
            Expr::Int(value) => (value.value.clone(), 100),
            Expr::Float(value) => (value.value.clone(), 100),
            Expr::String(value) => (self.string_literal(&value.value), 100),
            Expr::Char(value) => (format!("'{}'", value.value), 100),
            Expr::Bool(value) => (value.value.to_string(), 100),
            Expr::Name(value) => (value.value.clone(), 100),
            Expr::Wildcard(_) => ("_".to_string(), 100),
            Expr::Unary { op, expr, .. } => {
                let op_str = match op {
                    UnaryOp::Negate => "-",
                    UnaryOp::Not => "!",
                };
                let operand = self.expr(expr, 91, base_indent);
                (format!("{op_str}{operand}"), 90)
            }
            Expr::Binary {
                left, op, right, ..
            } => {
                let (op_str, prec, left_threshold, right_threshold) = binary_op_info(*op);
                let left_expr = self.expr(left, left_threshold, base_indent);
                let right_expr = self.expr(right, right_threshold, base_indent);
                (format!("{left_expr} {op_str} {right_expr}"), prec)
            }
            Expr::Call {
                callee,
                type_args,
                args,
                ..
            } => {
                let is_json_parse = matches!(
                    callee.as_ref(),
                    Expr::Field { target, field, .. }
                        if field.value == "parse"
                            && matches!(target.as_ref(), Expr::Name(name) if name.value == "json")
                );
                let callee = self.expr(callee, 100, base_indent);
                let type_args = if type_args.is_empty() {
                    String::new()
                } else {
                    let inner = type_args
                        .iter()
                        .map(|arg| self.type_(arg))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("[{inner}]")
                };
                let args = args
                    .iter()
                    .enumerate()
                    .map(|(index, arg)| {
                        if is_json_parse
                            && index == 1
                            && matches!(
                                arg,
                                Expr::String(literal)
                                    if literal.value.text == "__keel_json_tolerant"
                            )
                        {
                            "mode: .tolerant".to_string()
                        } else {
                            self.expr(arg, 0, base_indent)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                (format!("{callee}{type_args}({args})"), 100)
            }
            Expr::Field { target, field, .. } => {
                let target = self.expr(target, 100, base_indent);
                (format!("{target}.{}", field.value), 100)
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                let receiver = self.expr(receiver, 100, base_indent);
                let args = args
                    .iter()
                    .map(|arg| self.expr(arg, 0, base_indent))
                    .collect::<Vec<_>>()
                    .join(", ");
                (format!("{receiver}.{}({args})", method.value), 100)
            }
            Expr::StructLiteral {
                name,
                type_args,
                fields,
                ..
            } => {
                let type_args = if type_args.is_empty() {
                    String::new()
                } else {
                    let inner = type_args
                        .iter()
                        .map(|arg| self.type_(arg))
                        .collect::<Vec<_>>()
                        .join(", ");
                    format!("[{inner}]")
                };
                let fields = fields
                    .iter()
                    .map(|field| self.struct_literal_field(field, base_indent))
                    .collect::<Vec<_>>()
                    .join(", ");
                (format!("{}{} {{ {fields} }}", name.value, type_args), 100)
            }
            Expr::Block(block) => (self.inline_block(block, base_indent), 10),
            Expr::If {
                condition,
                then_block,
                else_branch,
                ..
            } => {
                let mut result = format!(
                    "if {} {}",
                    self.expr(condition, 0, base_indent),
                    self.inline_block(then_block, base_indent)
                );
                if let Some(else_branch) = else_branch {
                    result.push_str(&format!(" else {}", self.expr(else_branch, 0, base_indent)));
                }
                (result, 10)
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                let arms = arms
                    .iter()
                    .map(|arm| self.match_arm(arm, base_indent + 1))
                    .collect::<Vec<_>>()
                    .join("\n");
                let close = self.indent_line("}", base_indent);
                let result = format!(
                    "match {} {{\n{arms}\n{close}",
                    self.expr(scrutinee, 0, base_indent)
                );
                (result, 10)
            }
            Expr::While {
                condition, body, ..
            } => {
                let result = format!(
                    "while {} {}",
                    self.expr(condition, 0, base_indent),
                    self.inline_block(body, base_indent)
                );
                (result, 10)
            }
            Expr::Scope { deadline, body, .. } => {
                let options = deadline
                    .as_ref()
                    .map(|expr| format!("(deadline: {})", self.expr(expr, 0, base_indent)))
                    .unwrap_or_default();
                (
                    format!("scope{options} {}", self.inline_block(body, base_indent)),
                    10,
                )
            }
            Expr::Spawn { expr, .. } => {
                let inner = self.expr(expr, 90, base_indent);
                (format!("spawn {inner}"), 90)
            }
            Expr::Question { expr, .. } => {
                let inner = self.expr(expr, 100, base_indent);
                (format!("{inner}?"), 100)
            }
            Expr::Catch {
                expr,
                error_name,
                arms,
                ..
            } => {
                let arms = arms
                    .iter()
                    .map(|arm| self.match_arm(arm, base_indent + 1))
                    .collect::<Vec<_>>()
                    .join("\n");
                let close = self.indent_line("}", base_indent);
                let result = format!(
                    "{} catch {} {{\n{arms}\n{close}",
                    self.expr(expr, 0, base_indent),
                    error_name.value
                );
                (result, 10)
            }
            Expr::Return {
                value: Some(value), ..
            } => (format!("return {}", self.expr(value, 0, base_indent)), 10),
            Expr::Return { value: None, .. } => ("return".to_string(), 10),
        }
    }

    fn string_literal(&self, literal: &crate::StringLiteral) -> String {
        let mut text = literal.text.clone();
        for (index, interpolation) in literal.interpolations.iter().enumerate() {
            let needle = format!("{{{}}}", interpolation.value);
            if let Some(position) = text.find(&needle) {
                let placeholder = format!("\x00{index}\x00");
                text.replace_range(position..position + needle.len(), &placeholder);
            }
        }
        text = text.replace('{', "{{").replace('}', "}}");
        for (index, interpolation) in literal.interpolations.iter().enumerate() {
            let placeholder = format!("\x00{index}\x00");
            let marker = format!("{{{}}}", interpolation.value);
            text = text.replace(&placeholder, &marker);
        }
        format!("\"{text}\"")
    }

    fn inline_block(&self, block: &Block, base_indent: usize) -> String {
        let content_indent = base_indent + 1;
        let mut result = "{\n".to_string();
        for statement in &block.statements {
            let line = match statement {
                Stmt::Expr(expr) => self.expr(expr, 0, content_indent),
                other => {
                    let mut printer = Printer::new();
                    printer.indent = content_indent;
                    printer.stmt(other);
                    printer.finish().trim_end().to_string()
                }
            };
            result.push_str(&self.indent_line(&line, content_indent));
            result.push('\n');
        }
        result.push_str(&self.indent_line("}", base_indent));
        result
    }

    fn match_arm(&self, arm: &MatchArm, base_indent: usize) -> String {
        let pattern = self.pattern(&arm.pattern);
        let mut line = pattern;
        if let Some(guard) = &arm.guard {
            line.push_str(&format!(" if {}", self.expr(guard, 0, base_indent)));
        }
        line.push_str(&format!(" => {}", self.expr(&arm.value, 0, base_indent)));
        self.indent_line(&line, base_indent)
    }

    fn pattern(&self, pattern: &Pattern) -> String {
        match pattern {
            Pattern::Wildcard(_) => "_".to_string(),
            Pattern::Name { name, args, .. } => {
                let mut result = name.value.clone();
                if !args.is_empty() {
                    let args = args
                        .iter()
                        .map(|arg| self.pattern(arg))
                        .collect::<Vec<_>>()
                        .join(", ");
                    result.push_str(&format!("({args})"));
                }
                result
            }
        }
    }

    fn struct_literal_field(&self, field: &StructLiteralField, base_indent: usize) -> String {
        format!(
            "{}: {}",
            field.name.value,
            self.expr(&field.value, 0, base_indent)
        )
    }

    fn indent_line(&self, text: &str, indent: usize) -> String {
        let mut result = String::new();
        for _ in 0..indent {
            result.push_str("    ");
        }
        result.push_str(text);
        result
    }

    fn line(&mut self, text: &str) {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        self.output.push_str(text);
        self.output.push('\n');
    }

    fn write(&mut self, text: &str) {
        self.output.push_str(text);
    }

    fn blank_line(&mut self) {
        self.output.push('\n');
    }

    fn indent(&mut self) {
        self.indent += 1;
    }

    fn dedent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }
}

fn binary_op_info(op: BinaryOp) -> (&'static str, u8, u8, u8) {
    match op {
        BinaryOp::Or => ("||", 40, 40, 40),
        BinaryOp::And => ("&&", 50, 50, 50),
        BinaryOp::Equal => ("==", 60, 61, 61),
        BinaryOp::NotEqual => ("!=", 60, 61, 61),
        BinaryOp::Less => ("<", 60, 61, 61),
        BinaryOp::LessEqual => ("<=", 60, 61, 61),
        BinaryOp::Greater => (">", 60, 61, 61),
        BinaryOp::GreaterEqual => (">=", 60, 61, 61),
        BinaryOp::Add => ("+", 70, 70, 70),
        BinaryOp::Subtract => ("-", 70, 70, 71),
        BinaryOp::Multiply => ("*", 80, 80, 80),
        BinaryOp::Divide => ("/", 80, 80, 81),
        BinaryOp::Remainder => ("%", 80, 80, 81),
    }
}

fn needs_parens(child_prec: u8, parent_prec: u8) -> bool {
    child_prec < parent_prec
}

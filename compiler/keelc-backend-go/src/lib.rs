//! Go source emitter for the Keel backend.
//!
//! Consumes the explicitly-typed, desugared KIR produced by `keelc-kir`.
//! The backend no longer performs type inference or AST traversal; it only
//! maps KIR constructs to readable Go source.

use keelc_kir::{
    BinaryOp, Block, EnumDecl, Expr, Field, FunctionDecl, Item, MatchArm, Method, Module, Pattern,
    Stmt, StringLiteral, StringPart, StructDecl, UnaryOp, Variant,
};
use keelc_types::TypeInfo;
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

pub fn emit_tests(module: &Module) -> Result<String, BackendError> {
    Emitter::new_for_tests(module).emit_test_runner()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StructInfo {
    name: String,
    fields: Vec<Field>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct InterfaceInfo {
    name: String,
    methods: Vec<Method>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ImplInfo {
    interface_name: String,
    type_name: String,
    methods: Vec<FunctionDecl>,
}

struct Emitter<'a> {
    module: &'a Module,
    structs: Vec<StructInfo>,
    struct_names: Vec<String>,
    enum_variant_names: Vec<String>,
    interfaces: Vec<InterfaceInfo>,
    interface_names: Vec<String>,
    impls: Vec<ImplInfo>,
    output: String,
    indent: usize,
    temp_index: usize,
}

impl<'a> Emitter<'a> {
    fn new(module: &'a Module) -> Self {
        let structs = collect_structs(module);
        let struct_names = structs.iter().map(|s| s.name.clone()).collect();
        let interfaces = collect_interfaces(module);
        let interface_names = interfaces.iter().map(|i| i.name.clone()).collect();
        let enum_variant_names = collect_enum_variant_names(module);
        Self {
            module,
            structs,
            struct_names,
            enum_variant_names,
            interfaces,
            interface_names,
            impls: collect_impls(module),
            output: String::new(),
            indent: 0,
            temp_index: 0,
        }
    }

    fn new_for_tests(module: &'a Module) -> Self {
        Self::new(module)
    }

    fn go_type(&self, ty: &TypeInfo) -> String {
        go_type(ty, &self.struct_names, &self.interface_names)
    }

    fn emit(mut self) -> Result<String, BackendError> {
        self.line("package main")?;
        self.line("")?;
        self.line("import \"fmt\"")?;
        self.line("")?;
        self.emit_runtime()?;

        for interface in self.interfaces.clone() {
            self.emit_interface_decl(&interface)?;
            self.line("")?;
        }

        for item in &self.module.items {
            match item {
                Item::Struct(decl) => self.emit_struct_decl(decl)?,
                Item::Enum(decl) => self.emit_enum_decl(decl)?,
                Item::Function(_) | Item::Interface(_) | Item::Impl(_) | Item::Test(_) => {}
            }
        }

        for impl_decl in self.impls.clone() {
            for method in &impl_decl.methods {
                self.emit_impl_method(&impl_decl, method)?;
                self.line("")?;
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

    fn emit_test_runner(mut self) -> Result<String, BackendError> {
        self.line("package main")?;
        self.line("")?;
        self.line("import \"fmt\"")?;
        self.line("import \"os\"")?;
        self.line("")?;
        self.emit_runtime()?;

        for interface in self.interfaces.clone() {
            self.emit_interface_decl(&interface)?;
            self.line("")?;
        }

        for item in &self.module.items {
            match item {
                Item::Struct(decl) => self.emit_struct_decl(decl)?,
                Item::Enum(decl) => self.emit_enum_decl(decl)?,
                Item::Function(_) | Item::Interface(_) | Item::Impl(_) | Item::Test(_) => {}
            }
        }

        for impl_decl in self.impls.clone() {
            for method in &impl_decl.methods {
                self.emit_impl_method(&impl_decl, method)?;
                self.line("")?;
            }
        }

        for item in &self.module.items {
            if let Item::Function(function) = item {
                if function.name == "main" {
                    continue;
                }
                self.emit_function(function)?;
                self.line("")?;
            }
        }

        self.line("func main() {")?;
        self.indent += 1;
        for item in &self.module.items {
            if let Item::Test(test) = item {
                let name_literal = go_string_literal(&test.name);
                self.line_fmt(format_args!("fmt.Printf(\"test %q ... \", {name_literal})"))?;
                self.line("func() {")?;
                self.indent += 1;
                self.emit_block_statements(&test.body, false)?;
                self.indent -= 1;
                self.line("}()")?;
                self.line("fmt.Println(\"ok\")")?;
            }
        }
        self.indent -= 1;
        self.line("}")?;

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

    fn emit_checked_op(
        &mut self,
        name: &str,
        ret: &str,
        op: &str,
        none_branch: &str,
        ok_prefix: &str,
    ) -> Result<(), BackendError> {
        self.line_fmt(format_args!(
            "func {name}(left int64, right int64) {ret} {{"
        ))?;
        self.indent += 1;
        self.line_fmt(format_args!("if right == 0 {{ {none_branch} }}"))?;
        if ok_prefix.is_empty() {
            self.line_fmt(format_args!("return left {op} right"))?;
        } else {
            self.line_fmt(format_args!("return {ok_prefix}(left {op} right)"))?;
        }
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        Ok(())
    }

    fn emit_struct_decl(&mut self, decl: &StructDecl) -> Result<(), BackendError> {
        self.line_fmt(format_args!("type {} struct {{", decl.name))?;
        self.indent += 1;
        for field in &decl.fields {
            let ty = self.go_type(&field.ty);
            self.line_fmt(format_args!("{} {}", field.name, ty))?;
        }
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        Ok(())
    }

    fn emit_interface_decl(&mut self, interface: &InterfaceInfo) -> Result<(), BackendError> {
        self.line_fmt(format_args!("type {} interface {{", interface.name))?;
        self.indent += 1;
        for method in &interface.methods {
            let params = method
                .params
                .iter()
                .map(|param| format!("{} {}", param.name, self.go_type(&param.ty)))
                .collect::<Vec<_>>()
                .join(", ");
            let ret = self.go_type(&method.return_type);
            if ret.is_empty() {
                self.line_fmt(format_args!("{}({})", method.name, params))?;
            } else {
                self.line_fmt(format_args!("{}({}) {}", method.name, params, ret))?;
            }
        }
        self.indent -= 1;
        self.line("}")?;
        Ok(())
    }

    fn emit_impl_method(
        &mut self,
        impl_decl: &ImplInfo,
        method: &FunctionDecl,
    ) -> Result<(), BackendError> {
        write!(
            self.output,
            "func (self {}) {}(",
            impl_decl.type_name, method.name
        )?;
        for (index, param) in method.params.iter().enumerate() {
            if index > 0 {
                self.output.push_str(", ");
            }
            write!(self.output, "{} {}", param.name, self.go_type(&param.ty))?;
        }
        self.output.push(')');
        if method.return_type != TypeInfo::Unit {
            self.output.push(' ');
            self.output.push_str(&self.go_type(&method.return_type));
        }
        self.output.push_str(" {\n");

        self.indent += 1;
        self.emit_block_statements(&method.body, method.return_type != TypeInfo::Unit)?;
        self.indent -= 1;

        self.line("}")?;
        Ok(())
    }

    fn emit_enum_decl(&mut self, decl: &EnumDecl) -> Result<(), BackendError> {
        self.line_fmt(format_args!("type {} = KeelEnum", decl.name))?;
        self.line("")?;
        for variant in &decl.variants {
            self.emit_variant_constructor(decl.name.clone(), variant)?;
            self.line("")?;
        }
        Ok(())
    }

    fn emit_variant_constructor(
        &mut self,
        _enum_name: String,
        variant: &Variant,
    ) -> Result<(), BackendError> {
        if variant.fields.is_empty() {
            self.line_fmt(format_args!(
                "var {} = KeelEnum{{tag: {:?}}}",
                variant.name, variant.name
            ))?;
            return Ok(());
        }

        write!(self.output, "func {}(", variant.name)?;
        for (index, field) in variant.fields.iter().enumerate() {
            if index > 0 {
                self.output.push_str(", ");
            }
            write!(self.output, "{} {}", field.name, self.go_type(&field.ty))?;
        }
        self.output.push_str(") KeelEnum {\n");
        self.indent += 1;
        let values = variant
            .fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        self.line_fmt(format_args!(
            "return KeelEnum{{tag: {:?}, values: []any{{{values}}}}}",
            variant.name
        ))?;
        self.indent -= 1;
        self.line("}")?;
        Ok(())
    }

    fn emit_function(&mut self, function: &FunctionDecl) -> Result<(), BackendError> {
        write!(self.output, "func {}", function.name)?;
        self.output.push('(');
        for (index, param) in function.params.iter().enumerate() {
            if index > 0 {
                self.output.push_str(", ");
            }
            write!(self.output, "{} {}", param.name, self.go_type(&param.ty))?;
        }
        self.output.push(')');

        if function.return_type != TypeInfo::Unit {
            self.output.push(' ');
            self.output.push_str(&self.go_type(&function.return_type));
        }
        self.output.push_str(" {\n");

        self.indent += 1;
        self.emit_block_statements(&function.body, function.return_type != TypeInfo::Unit)?;
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
                    self.line_fmt(format_args!("return {expr}"))?;
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
                let emitted = self.emit_expr(value)?;
                let expr = self.cast_typed_literal(value, &emitted)?;
                self.line_fmt(format_args!("{} := {expr}", name))?;
                Ok(())
            }
            Stmt::Var { name, ty } => {
                let ty = self.go_type(ty);
                self.line_fmt(format_args!("var {} {}", name, ty))?;
                Ok(())
            }
            Stmt::Assign { target, value } => {
                let target = self.emit_expr(target)?;
                let value = self.emit_expr(value)?;
                self.line_fmt(format_args!("{target} = {value}"))
            }
            Stmt::Return { value } => self.emit_return_stmt(value.as_ref()),
            Stmt::Expr(Expr::If {
                condition,
                then_block,
                else_block,
                ..
            }) => self.emit_if_stmt(condition, then_block, else_block),
            Stmt::Expr(Expr::While { condition, body }) => self.emit_while_stmt(condition, body),
            Stmt::Expr(Expr::Match {
                scrutinee, arms, ..
            }) => self.emit_match_stmt(scrutinee, arms),
            Stmt::Expr(Expr::Block(block)) => self.emit_block_statements(block, false),
            Stmt::Expr(expr) => {
                let expr = self.emit_expr(expr)?;
                self.line(&expr)
            }
            Stmt::Assert { value, line } => self.emit_assert(value, *line),
            Stmt::Break => self.line("break"),
            Stmt::Continue => self.line("continue"),
        }
    }

    fn emit_assert(&mut self, value: &Expr, line: usize) -> Result<(), BackendError> {
        let expr = self.emit_expr(value)?;
        self.line_fmt(format_args!("if !({expr}) {{"))?;
        self.indent += 1;
        self.line_fmt(format_args!(
            "fmt.Fprintf(os.Stderr, \"assertion failed at line %d\\n\", {line})"
        ))?;
        self.line("os.Exit(1)")?;
        self.indent -= 1;
        self.line("}")
    }

    fn emit_return_stmt(&mut self, value: Option<&Expr>) -> Result<(), BackendError> {
        if let Some(value) = value {
            let expr = self.emit_expr(value)?;
            self.line_fmt(format_args!("return {expr}"))
        } else {
            self.line("return")
        }
    }

    fn emit_expr(&mut self, expr: &Expr) -> Result<String, BackendError> {
        match expr {
            Expr::Int(value) => Ok(value.clone()),
            Expr::Float(value) => Ok(value.clone()),
            Expr::String(literal) => self.emit_string(literal),
            Expr::Char(value) => Ok(format!("{:?}", value)),
            Expr::Bool(value) => Ok(value.to_string()),
            Expr::Name(name) => Ok(name.clone()),
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
                Ok(format!("{target}.{field}"))
            }
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => self.emit_method_call(receiver, method, args),
            Expr::StructLiteral { name, fields, .. } => self.emit_struct_literal(name, fields),
            Expr::If {
                condition,
                then_block,
                else_block,
                ty,
            } => self.emit_if_expr(condition, then_block, else_block, ty),
            Expr::Match {
                scrutinee,
                arms,
                ty,
            } => self.emit_match_expr(scrutinee, arms, ty),
            Expr::While { condition, body } => self.emit_while_expr(condition, body),
            Expr::Block(block) => self.emit_block_expr(block),
            Expr::Payload { value, index, ty } => {
                let value = self.emit_expr(value)?;
                Ok(format!(
                    "{}.values[{}].({})",
                    value,
                    index,
                    self.go_type(ty)
                ))
            }
            Expr::Return { value } => {
                if let Some(value) = value {
                    let expr = self.emit_expr(value)?;
                    Ok(format!("return {expr}"))
                } else {
                    Ok("return".to_string())
                }
            }
        }
    }

    fn emit_method_call(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[Expr],
    ) -> Result<String, BackendError> {
        if matches!(receiver, Expr::Name(name) if name == "Float") && method == "from" {
            let arg = args
                .first()
                .ok_or_else(|| BackendError::unsupported("Float.from without argument"))?;
            let arg_expr = self.emit_expr(arg)?;
            return Ok(format!("float64({arg_expr})"));
        }
        let receiver_expr = self.emit_expr(receiver)?;
        let args = args
            .iter()
            .map(|arg| self.emit_expr(arg))
            .collect::<Result<Vec<_>, _>>()?
            .join(", ");
        Ok(format!("{receiver_expr}.{method}({args})"))
    }

    fn emit_call(&mut self, callee: &Expr, args: &[Expr]) -> Result<String, BackendError> {
        let callee_name = match callee {
            Expr::Name(name) => Some(name.as_str()),
            _ => None,
        };
        let constructor = callee_name.is_some_and(|name| {
            name == "Some"
                || name == "Ok"
                || name == "Err"
                || self
                    .enum_variant_names
                    .iter()
                    .any(|variant| variant == name)
        });
        let is_print = callee_name == Some("print");

        let mut emitted_args = Vec::new();
        if constructor {
            for arg in args {
                emitted_args.push(self.cast_constructor_arg(arg)?);
            }
        } else {
            for arg in args {
                emitted_args.push(self.emit_expr(arg)?);
            }
        }

        if is_print {
            return Ok(format!("fmt.Println({})", emitted_args.join(", ")));
        }
        if constructor {
            let name = callee_name.ok_or_else(|| {
                BackendError::unsupported("constructor call without named callee")
            })?;
            return Ok(format!("{}({})", name, emitted_args.join(", ")));
        }

        if let Expr::Field { target, field, .. } = callee {
            if matches!(target.as_ref(), Expr::Name(name) if name == "Float") && field == "from" {
                let arg = emitted_args
                    .first()
                    .ok_or_else(|| BackendError::unsupported("Float.from without argument"))?;
                return Ok(format!("float64({arg})"));
            }
        }

        let callee = self.emit_expr(callee)?;
        Ok(format!("{callee}({})", emitted_args.join(", ")))
    }

    fn cast_constructor_arg(&mut self, arg: &Expr) -> Result<String, BackendError> {
        let emitted = self.emit_expr(arg)?;
        match expr_ty(arg) {
            TypeInfo::Int => Ok(format!("int64({emitted})")),
            TypeInfo::Float => Ok(format!("float64({emitted})")),
            TypeInfo::Char => Ok(format!("rune({emitted})")),
            _ => Ok(emitted),
        }
    }

    /// Cast literal values at let bindings so Go infers the desired Keel type
    /// (int64 / float64 / rune) instead of the untyped-default int.
    fn cast_typed_literal(&self, expr: &Expr, emitted: &str) -> Result<String, BackendError> {
        match expr {
            Expr::Int(_) => Ok(format!("int64({emitted})")),
            Expr::Float(_) => Ok(format!("float64({emitted})")),
            Expr::Char(_) => Ok(format!("rune({emitted})")),
            _ => Ok(emitted.to_string()),
        }
    }

    fn emit_struct_literal(
        &mut self,
        name: &str,
        fields: &[(String, Expr)],
    ) -> Result<String, BackendError> {
        let Some(info) = self.structs.iter().find(|info| info.name == name).cloned() else {
            return Err(BackendError::unsupported(format!(
                "struct literal `{name}`"
            )));
        };
        let mut parts = Vec::new();
        for field in &info.fields {
            let value = if let Some((_, provided)) = fields.iter().find(|(n, _)| n == &field.name) {
                self.emit_expr(provided)?
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
        else_block: &Block,
    ) -> Result<(), BackendError> {
        let condition = self.emit_expr(condition)?;
        self.line_fmt(format_args!("if {condition} {{"))?;
        self.indent += 1;
        self.emit_block_statements(then_block, false)?;
        self.indent -= 1;
        if else_block.statements.is_empty() && else_block.ty == TypeInfo::Unit {
            self.line("}")?;
        } else {
            self.line("} else {")?;
            self.indent += 1;
            self.emit_block_statements(else_block, false)?;
            self.indent -= 1;
            self.line("}")?;
        }
        Ok(())
    }

    fn emit_while_stmt(&mut self, condition: &Expr, body: &Block) -> Result<(), BackendError> {
        let condition = self.emit_expr(condition)?;
        self.line_fmt(format_args!("for {condition} {{"))?;
        self.indent += 1;
        self.emit_block_statements(body, false)?;
        self.indent -= 1;
        self.line("}")
    }

    fn emit_match_stmt(&mut self, scrutinee: &Expr, arms: &[MatchArm]) -> Result<(), BackendError> {
        let (temp, scrutinee_expr) = match scrutinee {
            Expr::Name(name) => (name.clone(), name.clone()),
            _ => {
                let temp = self.next_temp();
                let expr = self.emit_expr(scrutinee)?;
                (temp, expr)
            }
        };
        if !matches!(scrutinee, Expr::Name(_)) {
            self.line_fmt(format_args!("{temp} := {scrutinee_expr}"))?;
        }
        for arm in arms {
            self.emit_match_arm_stmt(&temp, arm)?;
        }
        Ok(())
    }

    fn emit_match_arm_stmt(&mut self, temp: &str, arm: &MatchArm) -> Result<(), BackendError> {
        match &arm.pattern {
            Pattern::Wildcard => self.line("if true {")?,
            Pattern::Name { name, .. } => {
                self.line_fmt(format_args!("if {temp}.tag == {:?} {{", name))?;
            }
        }
        self.indent += 1;
        self.emit_pattern_bindings(temp, &arm.pattern)?;
        if let Some(guard) = &arm.guard {
            let guard = self.emit_expr(guard)?;
            self.line_fmt(format_args!("if {guard} {{"))?;
            self.indent += 1;
            self.emit_stmt(&Stmt::Expr(arm.value.clone()))?;
            self.indent -= 1;
            self.line("}")?;
        } else {
            self.emit_stmt(&Stmt::Expr(arm.value.clone()))?;
        }
        self.indent -= 1;
        self.line("}")
    }

    fn emit_pattern_bindings(&mut self, temp: &str, pattern: &Pattern) -> Result<(), BackendError> {
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
                    let payload = format!("{}.values[{}].({})", temp, index, self.go_type(&ty));
                    self.line_fmt(format_args!("{} := {payload}", name))?;
                    self.line_fmt(format_args!("_ = {}", name))?;
                }
            }
        }
        Ok(())
    }

    fn emit_if_expr(
        &mut self,
        condition: &Expr,
        then_block: &Block,
        else_block: &Block,
        ty: &TypeInfo,
    ) -> Result<String, BackendError> {
        let condition = self.emit_expr(condition)?;
        if *ty == TypeInfo::Unit {
            let then_body = self.emit_statement_block(then_block)?;
            let else_body = self.emit_statement_block(else_block)?;
            return Ok(format!(
                "func() {{ if {condition} {{ {then_body} }} else {{ {else_body} }} }}()"
            ));
        }
        let go_ty = self.go_type(ty);
        let then_body = self.emit_returning_block(then_block)?;
        let else_body = if else_block.statements.is_empty() {
            format!("return {}", zero_value(ty))
        } else {
            self.emit_returning_block(else_block)?
        };
        Ok(format!(
            "func() {go_ty} {{ if {condition} {{ {then_body} }} else {{ {else_body} }} }}()"
        ))
    }

    fn emit_statement_block(&mut self, block: &Block) -> Result<String, BackendError> {
        let mut output = String::new();
        for statement in &block.statements {
            output.push_str(&self.emit_inline_stmt(statement)?);
            output.push(' ');
        }
        Ok(output)
    }

    fn emit_while_expr(&mut self, condition: &Expr, body: &Block) -> Result<String, BackendError> {
        let condition = self.emit_expr(condition)?;
        let body = self.emit_returning_block(body)?;
        Ok(format!("func() {{ for {condition} {{ {body} }} }}()"))
    }

    fn emit_match_expr(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchArm],
        ty: &TypeInfo,
    ) -> Result<String, BackendError> {
        let returns_value = *ty != TypeInfo::Unit;
        let result_go_type = self.go_type(ty);
        let (temp, scrutinee_expr) = match scrutinee {
            Expr::Name(name) => (name.clone(), String::new()),
            _ => {
                let temp = self.next_temp();
                let expr = self.emit_expr(scrutinee)?;
                (temp.clone(), format!("{temp} := {expr}; "))
            }
        };
        let mut out = String::new();
        if returns_value {
            write!(out, "func() {result_go_type} {{ ")?;
        } else {
            out.push_str("func() { ");
        }
        out.push_str(&scrutinee_expr);
        for arm in arms {
            self.write_match_arm(&mut out, &temp, ty, arm)?;
        }
        if returns_value {
            write!(out, "return {}; ", zero_value(ty))?;
        }
        out.push_str("}()");
        Ok(out)
    }

    fn write_match_arm(
        &mut self,
        out: &mut String,
        temp: &str,
        result_ty: &TypeInfo,
        arm: &MatchArm,
    ) -> Result<(), BackendError> {
        match &arm.pattern {
            Pattern::Wildcard => out.push_str("if true { "),
            Pattern::Name { name, .. } => {
                write!(out, "if {temp}.tag == {:?} {{ ", name)?;
            }
        }

        if let Pattern::Name {
            args,
            payload_types,
            ..
        } = &arm.pattern
        {
            for (index, arg) in args.iter().enumerate() {
                let ty = payload_types
                    .get(index)
                    .cloned()
                    .unwrap_or(TypeInfo::Unknown);
                if let Pattern::Name { name, .. } = arg {
                    let payload = format!("{}.values[{}].({})", temp, index, self.go_type(&ty));
                    write!(out, "{} := {}; _ = {}; ", name, payload, name)?;
                }
            }
        }

        if let Some(guard) = &arm.guard {
            let guard = self.emit_expr(guard)?;
            write!(out, "if {guard} {{ ")?;
            self.write_match_arm_value(out, result_ty, &arm.value)?;
            out.push_str("}; ");
        } else {
            self.write_match_arm_value(out, result_ty, &arm.value)?;
        }
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

    fn emit_block_expr(&mut self, block: &Block) -> Result<String, BackendError> {
        if block.ty == TypeInfo::Unit {
            let body = self.emit_statement_block(block)?;
            return Ok(format!("func() {{ {body} }}()"));
        }
        let ty = self.go_type(&block.ty);
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
            Stmt::Return { value } => {
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
                let emitted = self.emit_expr(value)?;
                let expr = self.cast_typed_literal(value, &emitted)?;
                Ok(format!("{} := {};", name, expr))
            }
            Stmt::Var { name, ty } => {
                let ty = self.go_type(ty);
                Ok(format!("var {} {};", name, ty))
            }
            Stmt::Assign { target, value } => Ok(format!(
                "{} = {};",
                self.emit_expr(target)?,
                self.emit_expr(value)?
            )),
            Stmt::Expr(expr) => Ok(format!("{};", self.emit_expr(expr)?)),
            Stmt::Return { value } => {
                if let Some(value) = value {
                    Ok(format!("return {};", self.emit_expr(value)?))
                } else {
                    Ok("return;".to_string())
                }
            }
            Stmt::Assert { value, line } => self.emit_assert_inline(value, *line),
            Stmt::Break | Stmt::Continue => Err(BackendError::unsupported(
                "break/continue in block expressions",
            )),
        }
    }

    fn emit_assert_inline(&mut self, value: &Expr, line: usize) -> Result<String, BackendError> {
        let expr = self.emit_expr(value)?;
        Ok(format!("if !({expr}) {{ fmt.Fprintf(os.Stderr, \"assertion failed at line %d\\n\", {line}); os.Exit(1) }}"))
    }

    fn emit_string(&mut self, literal: &StringLiteral) -> Result<String, BackendError> {
        if literal.parts.len() == 1 {
            if let StringPart::Text(text) = &literal.parts[0] {
                return Ok(format!("{:?}", text));
            }
        }
        let mut args = Vec::new();
        for part in &literal.parts {
            match part {
                StringPart::Text(text) => args.push(format!("{:?}", text)),
                StringPart::Expr(expr) => args.push(self.emit_expr(expr)?),
            }
        }
        if args.is_empty() {
            return Ok("\"\"".to_string());
        }
        Ok(format!("fmt.Sprint({})", args.join(", ")))
    }

    fn line(&mut self, text: &str) -> Result<(), BackendError> {
        self.line_fmt(format_args!("{text}"))
    }

    fn line_fmt(&mut self, args: fmt::Arguments<'_>) -> Result<(), BackendError> {
        for _ in 0..self.indent {
            self.output.push_str("    ");
        }
        writeln!(self.output, "{args}")?;
        Ok(())
    }

    fn next_temp(&mut self) -> String {
        let temp = format!("__keel_tmp_{}", self.temp_index);
        self.temp_index += 1;
        temp
    }
}

fn collect_structs(module: &Module) -> Vec<StructInfo> {
    let mut structs = Vec::new();
    for item in &module.items {
        if let Item::Struct(decl) = item {
            structs.push(StructInfo {
                name: decl.name.clone(),
                fields: decl.fields.clone(),
            });
        }
    }
    structs.sort_by(|left, right| left.name.cmp(&right.name));
    structs
}

fn collect_enum_variant_names(module: &Module) -> Vec<String> {
    let mut names = Vec::new();
    for item in &module.items {
        if let Item::Enum(decl) = item {
            for variant in &decl.variants {
                names.push(variant.name.clone());
            }
        }
    }
    names.sort();
    names
}

fn collect_interfaces(module: &Module) -> Vec<InterfaceInfo> {
    let mut interfaces = Vec::new();
    for item in &module.items {
        if let Item::Interface(decl) = item {
            interfaces.push(InterfaceInfo {
                name: decl.name.clone(),
                methods: decl.methods.clone(),
            });
        }
    }
    interfaces.sort_by(|left, right| left.name.cmp(&right.name));
    interfaces
}

fn collect_impls(module: &Module) -> Vec<ImplInfo> {
    let mut impls = Vec::new();
    for item in &module.items {
        if let Item::Impl(decl) = item {
            impls.push(ImplInfo {
                interface_name: decl.interface_name.clone(),
                type_name: decl.type_name.clone(),
                methods: decl.methods.clone(),
            });
        }
    }
    impls.sort_by(|left, right| {
        left.type_name
            .cmp(&right.type_name)
            .then_with(|| left.interface_name.cmp(&right.interface_name))
    });
    impls
}

fn go_type(ty: &TypeInfo, struct_names: &[String], interface_names: &[String]) -> String {
    match ty {
        TypeInfo::Named(name) if struct_names.iter().any(|n| n == name) => name.clone(),
        TypeInfo::Named(name) if interface_names.iter().any(|n| n == name) => name.clone(),
        TypeInfo::Int => "int64".to_string(),
        TypeInfo::Float => "float64".to_string(),
        TypeInfo::Bool => "bool".to_string(),
        TypeInfo::String => "string".to_string(),
        TypeInfo::Char => "rune".to_string(),
        TypeInfo::Unit => String::new(),
        TypeInfo::Named(_) | TypeInfo::Generic { .. } | TypeInfo::Union(_) => {
            "KeelEnum".to_string()
        }
        TypeInfo::Interface(name) => name.clone(),
        TypeInfo::Unknown => "any".to_string(),
    }
}

fn zero_value(ty: &TypeInfo) -> &'static str {
    match ty {
        TypeInfo::Int | TypeInfo::Float | TypeInfo::Char => "0",
        TypeInfo::Bool => "false",
        TypeInfo::String => "\"\"",
        TypeInfo::Unit => "",
        TypeInfo::Named(_) | TypeInfo::Generic { .. } | TypeInfo::Union(_) => "KeelEnum{}",
        TypeInfo::Interface(_) => "nil",
        TypeInfo::Unknown => "nil",
    }
}

fn go_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            ch => out.push(ch),
        }
    }
    out.push('"');
    out
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

fn expr_ty(expr: &Expr) -> TypeInfo {
    match expr {
        Expr::Int(_) => TypeInfo::Int,
        Expr::Float(_) => TypeInfo::Float,
        Expr::String(_) => TypeInfo::String,
        Expr::Char(_) => TypeInfo::Char,
        Expr::Bool(_) => TypeInfo::Bool,
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
        Expr::Block(block) => block.ty.clone(),
    }
}

impl From<fmt::Error> for BackendError {
    fn from(_: fmt::Error) -> Self {
        Self {
            message: "failed to write generated Go source".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{emit, emit_tests};
    use keelc_parse::parse;
    use keelc_span::SourceId;

    #[test]
    fn emits_test_runner_with_assertion_check() {
        let source = r#"test "addition holds" {
    assert 1 + 1 == 2
}
"#;
        let output = parse(SourceId::new(0), source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let kir = keelc_kir::lower::lower(&output.module, source);
        assert!(kir.diagnostics.is_empty(), "{kir:?}");
        let go = emit_tests(&kir.module).expect("emission should succeed");
        assert!(go.contains("import \"fmt\""));
        assert!(go.contains("import \"os\""));
        assert!(go.contains("func main() {"));
        assert!(go.contains(r#"fmt.Printf("test %q ... ", "addition holds")"#));
        assert!(go.contains("fmt.Println(\"ok\")"));
        assert!(go.contains("if !(((1 + 1) == 2)) {"));
        assert!(go.contains("fmt.Fprintf(os.Stderr, \"assertion failed at line %d\\n\", 2)"));
        assert!(go.contains("os.Exit(1)"));
    }

    #[test]
    fn test_runner_excludes_user_main() {
        let source = r#"fn main() {
    print("hello")
}

test "example" {
    assert true
}
"#;
        let output = parse(SourceId::new(0), source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let kir = keelc_kir::lower::lower(&output.module, source);
        assert!(kir.diagnostics.is_empty(), "{kir:?}");
        let go = emit_tests(&kir.module).expect("emission should succeed");
        let matches: Vec<_> = go
            .lines()
            .filter(|line| line.starts_with("func main()"))
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "test runner must define exactly one main function"
        );
    }

    #[test]
    fn emits_simple_function() {
        let source = r#"fn main() {
    print("hello")
}
"#;
        let output = parse(SourceId::new(0), source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let kir = keelc_kir::lower::lower(&output.module, source);
        assert!(kir.diagnostics.is_empty(), "{kir:?}");
        let go = emit(&kir.module).expect("emission should succeed");
        assert!(go.contains("package main"));
        assert!(go.contains("import \"fmt\""));
        assert!(go.contains("func main() {"));
        assert!(go.contains("fmt.Println(\"hello\")"));
    }

    #[test]
    fn emits_struct_decl() {
        let source = r#"struct Point {
    x: Int
    y: Int
}

fn main() -> Unit {}
"#;
        let output = parse(SourceId::new(0), source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let kir = keelc_kir::lower::lower(&output.module, source);
        assert!(kir.diagnostics.is_empty(), "{kir:?}");
        let go = emit(&kir.module).expect("emission should succeed");
        assert!(go.contains("type Point struct {"));
        assert!(go.contains("x int64"));
        assert!(go.contains("y int64"));
    }

    #[test]
    fn emits_string_interpolation() {
        let source = r#"fn main() {
    print("{1 + 2}")
}
"#;
        let output = parse(SourceId::new(0), source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let kir = keelc_kir::lower::lower(&output.module, source);
        assert!(kir.diagnostics.is_empty(), "{kir:?}");
        let go = emit(&kir.module).expect("emission should succeed");
        assert!(go.contains("fmt.Sprint((1 + 2))"));
    }

    #[test]
    fn emits_match_expression() {
        let source = r#"fn main() {
    let x = Some(1)
    match x {
        Some(n) => print("{n}")
        other => print("none")
    }
}
"#;
        let output = parse(SourceId::new(0), source);
        assert!(output.diagnostics.is_empty(), "{output:?}");
        let kir = keelc_kir::lower::lower(&output.module, source);
        assert!(kir.diagnostics.is_empty(), "{kir:?}");
        let go = emit(&kir.module).expect("emission should succeed");
        assert!(go.contains("if x.tag == \"Some\" {"));
        assert!(go.contains("fmt.Println(\"none\")"));
    }
}

//! Go source emitter for the Keel backend.
//!
//! Consumes the explicitly-typed, desugared KIR produced by `keelc-kir`.
//! The backend no longer performs type inference or AST traversal; it only
//! maps KIR constructs to readable Go source.

mod analysis;
mod json;
mod runtime;
mod types;

use crate::analysis::{
    collect_enum_variant_names, collect_impls, collect_interfaces, collect_structs, expr_ty,
    module_uses_concurrency, module_uses_http, module_uses_http_serve, module_uses_json,
    module_uses_log, ImplInfo, InterfaceInfo, StructInfo,
};
use crate::types::{
    go_binary_op, go_string_literal, go_type, json_type_name, primitive_box_name,
    primitive_underlying, zero_value,
};
use keelc_kir::{
    BinaryOp, Block, EnumDecl, Expr, FunctionDecl, Item, MatchArm, Module, Pattern, Stmt,
    StringLiteral, StringPart, StructDecl, UnaryOp, Variant,
};
use keelc_types::infer::{task_inner, task_value_type};
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

struct Emitter<'a> {
    module: &'a Module,
    structs: Vec<StructInfo>,
    struct_names: Vec<String>,
    enum_variant_names: Vec<String>,
    interfaces: Vec<InterfaceInfo>,
    interface_names: Vec<String>,
    impls: Vec<ImplInfo>,
    uses_concurrency: bool,
    uses_json: bool,
    uses_http: bool,
    uses_http_serve: bool,
    uses_log: bool,
    json_types: Vec<TypeInfo>,
    task_values: Vec<Vec<(String, TypeInfo)>>,
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
        let uses_concurrency = module_uses_concurrency(module);
        let uses_json = module_uses_json(module);
        let uses_http = module_uses_http(module);
        let uses_http_serve = module_uses_http_serve(module);
        let uses_log = module_uses_log(module);
        Self {
            module,
            structs,
            struct_names,
            enum_variant_names,
            interfaces,
            interface_names,
            impls: collect_impls(module),
            uses_concurrency,
            uses_json,
            uses_http,
            uses_http_serve,
            uses_log,
            json_types: Vec::new(),
            task_values: Vec::new(),
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
        self.emit_imports(false)?;
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

        self.emit_impls()?;

        for item in &self.module.items {
            if let Item::Function(function) = item {
                self.emit_function(function)?;
                self.line("")?;
            }
        }

        self.emit_json_codecs()?;

        Ok(self.output)
    }

    fn emit_test_runner(mut self) -> Result<String, BackendError> {
        self.line("package main")?;
        self.line("")?;
        self.emit_imports(true)?;
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

        self.emit_impls()?;

        for item in &self.module.items {
            if let Item::Function(function) = item {
                if function.name == "main" {
                    continue;
                }
                self.emit_function(function)?;
                self.line("")?;
            }
        }

        self.emit_json_codecs()?;

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

    fn emit_imports(&mut self, include_os: bool) -> Result<(), BackendError> {
        let mut imports = vec!["fmt"];
        if include_os {
            imports.push("os");
        }
        if self.uses_concurrency {
            imports.push("context");
            imports.push("sync");
            imports.push("time");
        }
        if self.uses_json {
            imports.push("encoding/json");
            imports.push("math");
            imports.push("strconv");
            imports.push("strings");
        }
        if self.uses_http_serve {
            imports.push("net/http");
        }
        if self.uses_json || self.uses_http_serve {
            imports.push("io");
        }
        imports.sort();
        imports.dedup();
        if imports.len() == 1 {
            self.line_fmt(format_args!("import {:?}", imports[0]))?;
            return Ok(());
        }
        self.line("import (")?;
        self.indent += 1;
        for import in imports {
            self.line_fmt(format_args!("{import:?}"))?;
        }
        self.indent -= 1;
        self.line(")")
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
            let mut params = method
                .params
                .iter()
                .map(|param| format!("{} {}", param.name, self.go_type(&param.ty)))
                .collect::<Vec<_>>();
            if self.uses_concurrency {
                params.insert(0, "__keel_ctx context.Context".to_string());
            }
            let params = params.join(", ");
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
        if self.uses_concurrency {
            self.output.push_str("__keel_ctx context.Context");
        }
        for (index, param) in method.params.iter().enumerate() {
            if index > 0 || self.uses_concurrency {
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
        if self.uses_concurrency {
            self.line("_ = __keel_ctx")?;
        }
        self.emit_block_statements(&method.body, method.return_type != TypeInfo::Unit)?;
        self.indent -= 1;

        self.line("}")?;
        Ok(())
    }

    /// Emit every `impl`'s methods. Struct (and enum) impls keep Go receiver
    /// methods for nominal dispatch; primitive impls become boxed wrapper types
    /// because Go forbids methods on predeclared types like `int64`.
    fn emit_impls(&mut self) -> Result<(), BackendError> {
        for impl_decl in self.impls.clone() {
            if primitive_underlying(&impl_decl.type_name).is_some() {
                continue;
            }
            for method in &impl_decl.methods {
                self.emit_impl_method(&impl_decl, method)?;
                self.line("")?;
            }
        }
        self.emit_primitive_boxes()?;
        Ok(())
    }

    /// For each primitive type carrying at least one `impl`, emit a defined
    /// wrapper type (`type keelBox_Int int64`) plus every impl method, so the
    /// wrapper satisfies any interface bound structurally and can be passed
    /// where an erased type parameter expects its bound interface.
    fn emit_primitive_boxes(&mut self) -> Result<(), BackendError> {
        let mut primitives: Vec<String> = self
            .impls
            .iter()
            .filter(|info| primitive_underlying(&info.type_name).is_some())
            .map(|info| info.type_name.clone())
            .collect();
        primitives.sort();
        primitives.dedup();

        let impls = self.impls.clone();
        for primitive in primitives {
            let Some(underlying) = primitive_underlying(&primitive) else {
                continue;
            };
            let box_name = format!("keelBox_{primitive}");
            self.line_fmt(format_args!("type {box_name} {underlying}"))?;
            self.line("")?;
            let mut emitted = Vec::new();
            for impl_decl in &impls {
                if impl_decl.type_name != primitive {
                    continue;
                }
                for method in &impl_decl.methods {
                    if emitted.contains(&method.name) {
                        continue;
                    }
                    emitted.push(method.name.clone());
                    self.emit_box_method(&box_name, underlying, method)?;
                    self.line("")?;
                }
            }
        }
        Ok(())
    }

    fn emit_box_method(
        &mut self,
        box_name: &str,
        underlying: &str,
        method: &FunctionDecl,
    ) -> Result<(), BackendError> {
        write!(self.output, "func (recv {box_name}) {}(", method.name)?;
        if self.uses_concurrency {
            self.output.push_str("__keel_ctx context.Context");
        }
        for (index, param) in method.params.iter().enumerate() {
            if index > 0 || self.uses_concurrency {
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
        if self.uses_concurrency {
            self.line("_ = __keel_ctx")?;
        }
        // Rebind `self` to the raw primitive so method bodies operate on the
        // underlying value rather than the wrapper type.
        self.line_fmt(format_args!("self := {underlying}(recv)"))?;
        self.line("_ = self")?;
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
        let accepts_context = self.uses_concurrency && function.name != "main";
        if accepts_context {
            self.output.push_str("__keel_ctx context.Context");
        }
        for (index, param) in function.params.iter().enumerate() {
            if index > 0 || accepts_context {
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
        if self.uses_concurrency && function.name == "main" {
            self.line("__keel_ctx := context.Background()")?;
            self.line("_ = __keel_ctx")?;
        } else if accepts_context {
            self.line("_ = __keel_ctx")?;
        }
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
                self.line_fmt(format_args!("_ = {name}"))?;
                Ok(())
            }
            Stmt::Var { name, ty } => {
                let ty = if *ty == TypeInfo::Unit {
                    "struct{}".to_string()
                } else {
                    self.go_type(ty)
                };
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
            Stmt::Expr(Expr::Payload {
                ty: TypeInfo::Unit, ..
            }) => self.line("_ = struct{}{}"),
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
            Expr::Call {
                callee,
                type_args,
                args,
                ..
            } => self.emit_call(callee, type_args, args),
            Expr::Field { target, field, .. } => {
                if field == "value" {
                    if let Expr::Name(name) = target.as_ref() {
                        if let Some(value_ty) = self.task_value_type(name) {
                            let target = self.emit_expr(target)?;
                            if value_ty == TypeInfo::Unit {
                                return Ok("struct{}{}".to_string());
                            }
                            return Ok(format!("{target}.value.({})", self.go_type(&value_ty)));
                        }
                    }
                }
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
            Expr::Scope {
                deadline,
                body,
                ty,
                error_ty,
            } => self.emit_scope_expr(deadline.as_deref(), body, ty, error_ty.as_ref()),
            Expr::Spawn { .. } => Err(BackendError::unsupported(
                "`spawn` outside scope lowering context",
            )),
            Expr::Block(block) => self.emit_block_expr(block),
            Expr::Payload { value, index, ty } => {
                if *ty == TypeInfo::Unit {
                    return Ok("struct{}{}".to_string());
                }
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
                    if expr_ty(value) == TypeInfo::Unit {
                        Ok(format!("{expr}; return"))
                    } else {
                        Ok(format!("return {expr}"))
                    }
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
        if matches!(receiver, Expr::Name(name) if name == "time") {
            let arg = args.first().ok_or_else(|| {
                BackendError::unsupported(format!("time.{method} without argument"))
            })?;
            let arg = self.emit_expr(arg)?;
            return match method {
                "milliseconds" => Ok(format!("keelDuration({arg}, time.Millisecond)")),
                "seconds" => Ok(format!("keelDuration({arg}, time.Second)")),
                "sleep" => Ok(format!("keelSleep(__keel_ctx, {arg})")),
                _ => Err(BackendError::unsupported(format!("time.{method}"))),
            };
        }
        if matches!(receiver, Expr::Name(name) if name == "http") {
            return self.emit_http_call(method, args);
        }
        if matches!(receiver, Expr::Name(name) if name == "log") {
            return self.emit_log_call(method, args);
        }
        if matches!(receiver, Expr::Name(name) if name == "json") && method == "write" {
            let value = args
                .first()
                .ok_or_else(|| BackendError::unsupported("json.write without argument"))?;
            let value_type = expr_ty(value);
            self.register_json_type(&value_type);
            let value = self.emit_expr(value)?;
            return Ok(format!(
                "keelJSONEncode_{}({}, \"$\")",
                json_type_name(&value_type),
                value
            ));
        }
        let receiver_expr = self.emit_expr(receiver)?;
        let mut args = args
            .iter()
            .map(|arg| self.emit_expr(arg))
            .collect::<Result<Vec<_>, _>>()?;
        if self.uses_concurrency {
            args.insert(0, "__keel_ctx".to_string());
        }
        let args = args.join(", ");
        Ok(format!("{receiver_expr}.{method}({args})"))
    }

    fn emit_call(
        &mut self,
        callee: &Expr,
        type_args: &[TypeInfo],
        args: &[Expr],
    ) -> Result<String, BackendError> {
        if let Expr::Field { target, field, .. } = callee {
            if matches!(target.as_ref(), Expr::Name(name) if name == "json") && field == "parse" {
                let target_type = type_args.first().ok_or_else(|| {
                    BackendError::unsupported("json.parse without a type argument")
                })?;
                let input = args
                    .first()
                    .ok_or_else(|| BackendError::unsupported("json.parse without input"))?;
                self.register_json_type(target_type);
                let input = self.emit_expr(input)?;
                let tolerant = args.get(1).is_some_and(|arg| {
                    matches!(
                        arg,
                        Expr::String(literal)
                            if matches!(
                                literal.parts.as_slice(),
                                [StringPart::Text(text)] if text == "__keel_json_tolerant"
                            )
                    )
                });
                return Ok(format!(
                    "keelJSONParse_{}({}, {})",
                    json_type_name(target_type),
                    input,
                    tolerant
                ));
            }
            if matches!(target.as_ref(), Expr::Name(name) if name == "http") {
                return self.emit_http_call(field, args);
            }
            if matches!(target.as_ref(), Expr::Name(name) if name == "log") {
                return self.emit_log_call(field, args);
            }
        }
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
        if callee_name == Some("check_cancel") {
            return Ok("keelCheckCancel(__keel_ctx)".to_string());
        }

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
            return Ok(format!("keelPrint({})", emitted_args.join(", ")));
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

        let final_args = if let Some(params) = self.callee_param_types(callee) {
            args.iter()
                .zip(emitted_args)
                .enumerate()
                .map(|(index, (arg, emitted))| match params.get(index) {
                    Some(slot) => box_for_slot(slot, arg, emitted),
                    None => emitted,
                })
                .collect::<Vec<_>>()
        } else {
            emitted_args
        };
        let is_user_function = self.callee_param_types(callee).is_some();
        let callee = self.emit_expr(callee)?;
        let mut final_args = final_args;
        if self.uses_concurrency && is_user_function {
            final_args.insert(0, "__keel_ctx".to_string());
        }
        Ok(format!("{callee}({})", final_args.join(", ")))
    }

    /// Declared parameter types for a directly-named user function, used to box
    /// primitive arguments flowing into erased type-parameter / interface slots.
    fn callee_param_types(&self, callee: &Expr) -> Option<Vec<TypeInfo>> {
        let Expr::Name(name) = callee else {
            return None;
        };
        self.module.items.iter().find_map(|item| match item {
            Item::Function(function) if function.name == *name => Some(
                function
                    .params
                    .iter()
                    .map(|param| param.ty.clone())
                    .collect(),
            ),
            _ => None,
        })
    }

    fn task_value_type(&self, name: &str) -> Option<TypeInfo> {
        self.task_values
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev())
            .find(|(task_name, _)| task_name == name)
            .map(|(_, ty)| ty.clone())
    }

    fn register_json_type(&mut self, ty: &TypeInfo) {
        if !self.json_types.contains(ty) {
            self.json_types.push(ty.clone());
        }
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
                let emitted = self.emit_expr(provided)?;
                box_for_slot(&field.ty, provided, emitted)
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
                    let payload = if ty == TypeInfo::Unit {
                        "struct{}{}".to_string()
                    } else {
                        format!("{}.values[{}].({})", temp, index, self.go_type(&ty))
                    };
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
                    let payload = if ty == TypeInfo::Unit {
                        "struct{}{}".to_string()
                    } else {
                        format!("{}.values[{}].({})", temp, index, self.go_type(&ty))
                    };
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

    fn emit_scope_expr(
        &mut self,
        deadline: Option<&Expr>,
        body: &Block,
        ty: &TypeInfo,
        error_ty: Option<&TypeInfo>,
    ) -> Result<String, BackendError> {
        let return_ty = self.go_type(ty);
        let mut out = String::new();
        if return_ty.is_empty() {
            out.push_str("func() { ");
        } else {
            write!(out, "func() {return_ty} {{ ")?;
        }
        if let Some(deadline) = deadline {
            let deadline = self.emit_expr(deadline)?;
            write!(
                out,
                "ctx, cancel := context.WithTimeout(__keel_ctx, {deadline}); "
            )?;
        } else {
            out.push_str("ctx, cancel := context.WithCancel(__keel_ctx); ");
        }
        out.push_str("defer cancel(); __keel_ctx := ctx; var wg keelWaitGroup; ");
        if error_ty.is_some() {
            out.push_str(
                "var firstErr KeelEnum; firstErrIndex := int64(-1); var errMu keelMutex; _ = errMu; ",
            );
        }

        self.task_values.push(Vec::new());
        let Some((tail, prefix)) = body.statements.split_last() else {
            self.task_values.pop();
            if return_ty.is_empty() {
                out.push_str("}()");
            } else {
                write!(out, "return {}; }}()", zero_value(ty))?;
            }
            return Ok(out);
        };

        let mut spawn_index = 0usize;
        for statement in prefix {
            if let Stmt::Let {
                name,
                value: Expr::Spawn { expr, ty },
                ..
            } = statement
            {
                self.emit_scope_spawn(&mut out, name, expr, ty, spawn_index)?;
                spawn_index += 1;
            } else {
                out.push_str(&self.emit_inline_stmt(statement)?);
                out.push(' ');
            }
        }

        out.push_str("wg.Wait(); ");
        if error_ty.is_some() {
            out.push_str("if firstErrIndex >= 0 { return firstErr }; ");
            if deadline.is_some() {
                out.push_str("if ctx.Err() != nil { return Err(Cancelled) }; ");
            }
        }

        match tail {
            Stmt::Expr(expr) if error_ty.is_some() => {
                let value = self.cast_constructor_arg(expr)?;
                write!(out, "return Ok({value}); ")?;
            }
            Stmt::Expr(expr) if return_ty.is_empty() => {
                let expr = self.emit_expr(expr)?;
                write!(out, "{expr}; ")?;
            }
            Stmt::Expr(expr) => {
                let expr = self.emit_expr(expr)?;
                write!(out, "return {expr}; ")?;
            }
            _ => {
                out.push_str(&self.emit_inline_stmt(tail)?);
                if !return_ty.is_empty() {
                    write!(out, "return {}; ", zero_value(ty))?;
                }
            }
        }

        self.task_values.pop();
        out.push_str("}()");
        Ok(out)
    }

    fn emit_scope_spawn(
        &mut self,
        out: &mut String,
        name: &str,
        expr: &Expr,
        task_ty: &TypeInfo,
        spawn_index: usize,
    ) -> Result<(), BackendError> {
        let Some(inner_ty) = task_inner(task_ty) else {
            return Err(BackendError::unsupported("spawn without Task type"));
        };
        let value_ty = task_value_type(inner_ty);
        if let Some(scope) = self.task_values.last_mut() {
            scope.push((name.to_string(), value_ty.clone()));
        }

        write!(
            out,
            "var {name} keelTask; wg.Add(1); go func() {{ defer wg.Done(); "
        )?;
        let expr = self.emit_expr(expr)?;
        if inner_ty.result_parts().is_some() {
            let result_name = format!("__keel_task_result_{spawn_index}");
            write!(
                out,
                "{result_name} := {expr}; {name}.result = {result_name}; "
            )?;
            write!(
                out,
                "if {result_name}.tag == \"Err\" {{ errMu.Lock(); if firstErrIndex == -1 || int64({spawn_index}) < firstErrIndex {{ firstErr = {result_name}; firstErrIndex = int64({spawn_index}) }}; errMu.Unlock(); cancel(); return }}; "
            )?;
            write!(out, "{name}.value = {result_name}.values[0]; ")?;
        } else {
            write!(out, "{name}.value = {expr}; ")?;
        }
        out.push_str("}(); ");
        Ok(())
    }

    fn emit_http_call(&mut self, method: &str, args: &[Expr]) -> Result<String, BackendError> {
        match method {
            "ok" | "created" | "bad_request" | "conflict" | "internal_error" => {
                let body = args
                    .first()
                    .map(|a| self.emit_expr(a))
                    .unwrap_or(Ok("\"\"".to_string()))?;
                let status = match method {
                    "ok" => "200",
                    "created" => "201",
                    "bad_request" => "400",
                    "conflict" => "409",
                    "internal_error" => "500",
                    _ => unreachable!(),
                };
                Ok(format!(
                    "keelHTTPResponse{{status: {status}, body: {body}}}"
                ))
            }
            "no_content" | "not_found" => {
                let status = match method {
                    "no_content" => "204",
                    "not_found" => "404",
                    _ => unreachable!(),
                };
                Ok(format!("keelHTTPResponse{{status: {status}, body: \"\"}}"))
            }
            "serve" => {
                let port = args
                    .first()
                    .map(|a| self.emit_expr(a))
                    .unwrap_or(Ok("0".to_string()))?;
                let handler = args
                    .get(1)
                    .map(|a| self.emit_expr(a))
                    .unwrap_or(Ok("\"\"".to_string()))?;
                Ok(format!("keelHTTPServe({port}, {handler})"))
            }
            _ => Err(BackendError::unsupported(format!("http.{method}"))),
        }
    }

    fn emit_log_call(&mut self, method: &str, args: &[Expr]) -> Result<String, BackendError> {
        let msg = args
            .first()
            .map(|a| self.emit_expr(a))
            .unwrap_or(Ok("\"\"".to_string()))?;
        let go_func = match method {
            "info" => "keelLogInfo",
            "warn" => "keelLogWarn",
            "error" => "keelLogError",
            _ => return Err(BackendError::unsupported(format!("log.{method}"))),
        };
        Ok(format!("{go_func}({msg})"))
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
                Ok(format!("{name} := {expr}; _ = {name};"))
            }
            Stmt::Var { name, ty } => {
                let ty = if *ty == TypeInfo::Unit {
                    "struct{}".to_string()
                } else {
                    self.go_type(ty)
                };
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
                StringPart::Expr(expr) => {
                    let emitted = self.emit_expr(expr)?;
                    if expr_ty(expr) == TypeInfo::Char {
                        args.push(format!("string({emitted})"));
                    } else {
                        args.push(emitted);
                    }
                }
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
        let temp = format!("__keel_backend_tmp_{}", self.temp_index);
        self.temp_index += 1;
        temp
    }
}

fn box_for_slot(slot_ty: &TypeInfo, value: &Expr, emitted: String) -> String {
    if matches!(slot_ty, TypeInfo::Interface(_) | TypeInfo::TypeParam { .. }) {
        if let Some(box_name) = primitive_box_name(&expr_ty(value)) {
            return format!("{box_name}({emitted})");
        }
    }
    emitted
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
        assert!(go.contains("import ("));
        assert!(go.contains("\"fmt\""));
        assert!(go.contains("\"os\""));
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
        assert!(go.contains("keelPrint(\"hello\")"));
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
        assert!(go.contains("keelPrint(\"none\")"));
    }
}

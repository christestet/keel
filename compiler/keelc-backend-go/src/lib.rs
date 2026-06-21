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
    collect_enum_variant_names, collect_impls, collect_interfaces, collect_structs, collect_usage,
    expr_ty, ImplInfo, InterfaceInfo, StructInfo,
};
use crate::types::{
    go_binary_op, go_string_literal, go_type, json_type_name, primitive_box_name,
    primitive_underlying, zero_value,
};
use keelc_kir::{
    BinaryOp, Block, EnumDecl, Expr, Field, FunctionDecl, Item, MatchArm, Module, Pattern, Route,
    RouteHandler, Stmt, StringLiteral, StringPart, StructDecl, UnaryOp, Variant,
};
use keelc_types::infer::{task_inner, task_value_type};
use keelc_types::TypeInfo;
use std::fmt::{self, Write as _};

/// A config field type whose env parsing calls `strconv` (`Int`/`Float`, bare
/// or wrapped in `Option`). `Bool` uses `keelConfigBool`; `String`/`Secret`
/// need no parsing.
fn config_field_needs_strconv(ty: &TypeInfo) -> bool {
    match ty {
        TypeInfo::Int | TypeInfo::Float => true,
        TypeInfo::Generic { name, args } if name == "Option" => {
            args.first().is_some_and(config_field_needs_strconv)
        }
        _ => false,
    }
}

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

/// Map a `path_param<T>` / `query_param<T>` type argument to its runtime helper
/// suffix (`keelPathParamString`, `keelQueryParamInt`, …). M6 supports the
/// scalar wire types; richer types arrive with their own KDR.
fn is_result_type(ty: &TypeInfo) -> bool {
    matches!(ty, TypeInfo::Generic { name, .. } if name == "Result")
}

/// Conjoin pattern conditions; an empty set always matches.
fn join_conditions(conds: &[String]) -> String {
    if conds.is_empty() {
        "true".to_string()
    } else {
        conds.join(" && ")
    }
}

/// `(access.tag == "A" || access.tag == "B" ...)` — a typed binding's tag-set
/// membership test (KDR-0038).
fn tag_membership(access: &str, tags: &[String]) -> String {
    if tags.is_empty() {
        return "true".to_string();
    }
    let checks = tags
        .iter()
        .map(|tag| format!("{access}.tag == {tag:?}"))
        .collect::<Vec<_>>()
        .join(" || ");
    format!("({checks})")
}

fn request_param_suffix(ty: &TypeInfo) -> Result<&'static str, BackendError> {
    match ty {
        TypeInfo::String => Ok("String"),
        TypeInfo::Int => Ok("Int"),
        TypeInfo::Bool => Ok("Bool"),
        TypeInfo::Float => Ok("Float"),
        TypeInfo::Named(name) if name == "Uuid" => Ok("Uuid"),
        TypeInfo::Named(name) if name == "Timestamp" => Ok("Timestamp"),
        TypeInfo::Named(name) if name == "Email" => Ok("Email"),
        other => Err(BackendError::unsupported(format!(
            "request parameter of type `{other}`"
        ))),
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
    Emitter::new(module).emit_test_runner()
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
    uses_sql: bool,
    uses_config: bool,
    uses_uuid_new: bool,
    uses_timestamp_now: bool,
    config_needs_strconv: bool,
    json_types: Vec<TypeInfo>,
    config_types: Vec<TypeInfo>,
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
        let usage = collect_usage(module);
        let config_types = usage.config_types;
        // `strconv` is only used by Int/Float field parsing; Secret/String/Bool
        // fields never touch it, so importing it unconditionally would break the
        // Go build with an unused import.
        let config_needs_strconv = config_types.iter().any(|ty| {
            let TypeInfo::Named(name) = ty else {
                return false;
            };
            structs
                .iter()
                .find(|s| s.name == *name)
                .is_some_and(|s| s.fields.iter().any(|f| config_field_needs_strconv(&f.ty)))
        });
        Self {
            module,
            structs,
            struct_names,
            enum_variant_names,
            interfaces,
            interface_names,
            impls: collect_impls(module),
            uses_concurrency: usage.concurrency,
            uses_json: usage.json,
            uses_http: usage.http,
            uses_http_serve: usage.http_serve,
            uses_log: usage.log,
            uses_sql: usage.sql,
            uses_config: usage.config,
            uses_uuid_new: usage.uuid_new,
            uses_timestamp_now: usage.timestamp_now,
            config_needs_strconv,
            json_types: Vec::new(),
            config_types,
            task_values: Vec::new(),
            output: String::new(),
            indent: 0,
            temp_index: 0,
        }
    }

    fn go_type(&self, ty: &TypeInfo) -> String {
        go_type(ty, &self.struct_names, &self.interface_names)
    }

    fn emit(mut self) -> Result<String, BackendError> {
        let main_result = self.module.items.iter().any(|item| {
            matches!(item, Item::Function(f) if f.name == "main" && is_result_type(&f.return_type))
        });
        self.line("package main")?;
        self.line("")?;
        self.emit_imports(main_result)?;
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
        self.emit_config_loaders()?;
        self.emit_struct_from_rows()?;

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
        self.emit_struct_from_rows()?;

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
        }
        if self.uses_concurrency
            || self.uses_json
            || self.uses_http
            || self.uses_timestamp_now
            || self.uses_sql
        {
            imports.push("time");
        }
        if self.uses_json {
            imports.push("encoding/json");
            imports.push("math");
            imports.push("strconv");
            imports.push("strings");
        }
        if self.uses_uuid_new {
            imports.push("crypto/rand");
        }
        if self.uses_http_serve {
            imports.push("net/http");
            imports.push("net/url");
            imports.push("strconv");
            imports.push("strings");
        }
        if self.uses_sql {
            imports.push("database/sql");
            imports.push("strings");
        }
        if self.uses_log {
            imports.push("strings");
        }
        if self.uses_config {
            imports.push("os");
        }
        if self.config_needs_strconv {
            imports.push("strconv");
        }
        if self.uses_json || self.uses_http_serve {
            imports.push("io");
        }
        imports.sort();
        imports.dedup();
        // Blank-import the SQLite driver so database/sql can resolve the
        // "sqlite" driver name at runtime (KDR-0042).
        let blank_imports: &[&str] = if self.uses_sql {
            &["modernc.org/sqlite"]
        } else {
            &[]
        };
        if imports.len() == 1 && blank_imports.is_empty() {
            self.line_fmt(format_args!("import {:?}", imports[0]))?;
            return Ok(());
        }
        self.line("import (")?;
        self.indent += 1;
        for import in imports {
            self.line_fmt(format_args!("{import:?}"))?;
        }
        for blank in blank_imports {
            self.line_fmt(format_args!("_ {blank:?}"))?;
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
        if function.name == "main" && is_result_type(&function.return_type) {
            return self.emit_main_with_result(function);
        }
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

    /// `fn main() -> Result<Unit, E>` (keel-core §7): emit the body as
    /// `keelMain() KeelEnum`, then a Go `main` that prints the error to stderr
    /// and exits non-zero on `Err`.
    fn emit_main_with_result(&mut self, function: &FunctionDecl) -> Result<(), BackendError> {
        self.output.push_str("func keelMain() KeelEnum {\n");
        self.indent += 1;
        if self.uses_concurrency {
            self.line("__keel_ctx := context.Background()")?;
            self.line("_ = __keel_ctx")?;
        }
        self.emit_block_statements(&function.body, true)?;
        self.indent -= 1;
        self.line("}")?;
        self.line("")?;
        self.line("func main() {")?;
        self.indent += 1;
        self.line("if __keel_r := keelMain(); __keel_r.tag == \"Err\" {")?;
        self.indent += 1;
        self.line("fmt.Fprintln(os.Stderr, __keel_r.values[0])")?;
        self.line("os.Exit(1)")?;
        self.indent -= 1;
        self.line("}")?;
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
            // A discarded `?` result (e.g. `db.exec(...)?` for its effect only)
            // lowers to a bare payload expression; Go rejects unused values.
            Stmt::Expr(expr @ Expr::Payload { .. }) => {
                let expr = self.emit_expr(expr)?;
                self.line_fmt(format_args!("_ = {expr}"))
            }
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
            Expr::Unit => Ok("struct{}{}".to_string()),
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
                if field == "from_row" {
                    if let Expr::Name(name) = target.as_ref() {
                        if self.struct_names.contains(name) {
                            return Ok(format!("keelFromRow_{name}"));
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
                arg_types,
                ..
            } => self.emit_method_call(receiver, method, args, arg_types),
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
            Expr::Router { routes, .. } => self.emit_router(routes),
        }
    }

    /// Emit a `[]keelRoute{ ... }` literal (KDR-0031). Each entry pairs the route
    /// pattern with a Go handler func; the runtime splits method/path and extracts
    /// `{name}` path params at request time.
    fn emit_router(&mut self, routes: &[Route]) -> Result<String, BackendError> {
        let mut entries = Vec::new();
        for route in routes {
            let handler = match &route.handler {
                RouteHandler::Named(name) => name.clone(),
                RouteHandler::Closure { param, body } => {
                    let body = self.emit_expr(body)?;
                    format!("func({param} keelHTTPRequest) keelHTTPResponse {{ return {body} }}")
                }
            };
            entries.push(format!(
                "{{pattern: {}, handler: {handler}}}",
                go_string_literal(&route.pattern)
            ));
        }
        Ok(format!("[]keelRoute{{{}}}", entries.join(", ")))
    }

    /// Codegen for `std.sql` value methods (KDR-0029). Dispatched by method name,
    /// gated on `uses_sql` since the backend cannot re-derive a `Name`'s type.
    // ponytail: name-based dispatch, scoped to sql modules; upgrade to a typed
    // receiver check if a sql module ever shadows these names on a user type.
    fn emit_sql_method(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[Expr],
    ) -> Result<Option<String>, BackendError> {
        if !self.uses_sql {
            return Ok(None);
        }
        let func = match method {
            "query" => "keelSQLQuery",
            "query_one" => "keelSQLQueryOne",
            "exec" => "keelSQLExec",
            "migrate" => "keelSQLMigrate",
            "collect" => "keelSQLCollect",
            "map" => {
                let recv = self.emit_expr(receiver)?;
                let mapper = args
                    .first()
                    .ok_or_else(|| BackendError::unsupported("sql map without a function"))?;
                let mapper = self.emit_expr(mapper)?;
                return Ok(Some(format!(
                    "keelSQLMap({recv}, func(__keel_row keelSQLRow) any {{ return {mapper}(__keel_row) }})"
                )));
            }
            _ => return Ok(None),
        };
        let recv = self.emit_expr(receiver)?;
        if method == "collect" {
            return Ok(Some(format!("{func}({recv})")));
        }
        // migrate takes only the statement text; query/query_one/exec take the
        // query string followed by its bound parameters ($1, $2, ...).
        let emitted = args
            .iter()
            .map(|a| self.emit_expr(a))
            .collect::<Result<Vec<_>, _>>()?;
        if method == "migrate" {
            let stmt = emitted.first().cloned().unwrap_or_default();
            return Ok(Some(format!("{func}({recv}, {stmt})")));
        }
        let call_args = std::iter::once(recv).chain(emitted).collect::<Vec<_>>();
        Ok(Some(format!("{func}({})", call_args.join(", "))))
    }

    fn emit_method_call(
        &mut self,
        receiver: &Expr,
        method: &str,
        args: &[Expr],
        arg_types: &[TypeInfo],
    ) -> Result<String, BackendError> {
        if matches!(receiver, Expr::Name(name) if name == "Float") && method == "from" {
            let arg = args
                .first()
                .ok_or_else(|| BackendError::unsupported("Float.from without argument"))?;
            let arg_expr = self.emit_expr(arg)?;
            return Ok(format!("float64({arg_expr})"));
        }
        if matches!(receiver, Expr::Name(name) if name == "Uuid") && method == "new" {
            return Ok("keelUUIDNew()".to_string());
        }
        if matches!(receiver, Expr::Name(name) if name == "Timestamp") && method == "now" {
            return Ok("keelTimestampNow()".to_string());
        }
        if matches!(receiver, Expr::Name(name) if name == "sql") && method == "connect" {
            let arg = args
                .first()
                .ok_or_else(|| BackendError::unsupported("sql.connect without argument"))?;
            let arg = self.emit_expr(arg)?;
            return Ok(format!("keelSQLConnect({arg})"));
        }
        if let Some(call) = self.emit_sql_method(receiver, method, args)? {
            return Ok(call);
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
            let value_type = arg_types.first().cloned().unwrap_or_else(|| expr_ty(value));
            self.register_json_type(&value_type);
            let value = self.emit_expr(value)?;
            // Encoding a representable type is total (KDR-0040); the encoder
            // always returns Ok(string), so unwrap the payload directly.
            return Ok(format!(
                "keelJSONEncode_{}({}, \"$\").values[0].(string)",
                json_type_name(&value_type),
                value
            ));
        }
        // Derive Struct.from_row(row) -> Result<Struct, sql.Error> for any struct.
        if method == "from_row" {
            if let Expr::Name(name) = receiver {
                if self.struct_names.contains(name) {
                    let arg = args
                        .first()
                        .ok_or_else(|| BackendError::unsupported("from_row without argument"))?;
                    let arg_expr = self.emit_expr(arg)?;
                    return Ok(format!("keelFromRow_{name}({arg_expr})"));
                }
            }
        }
        // Option<T>.unwrap() -> T: extract the Some payload, abort on None (KDR-0039).
        if method == "unwrap" {
            if let TypeInfo::Generic {
                name,
                args: type_args,
            } = expr_ty(receiver)
            {
                if name == "Option" && type_args.len() == 1 {
                    let go_ty = self.go_type(&type_args[0]);
                    let recv = self.emit_expr(receiver)?;
                    return Ok(format!("keelOptionUnwrap({recv}).({go_ty})"));
                }
            }
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
            if matches!(target.as_ref(), Expr::Name(name) if name == "config") && field == "load" {
                let target_type = type_args.first().ok_or_else(|| {
                    BackendError::unsupported("config.load without a type argument")
                })?;
                self.register_config_type(target_type);
                return Ok(format!("keelConfigLoad_{}()", json_type_name(target_type)));
            }
            if matches!(target.as_ref(), Expr::Name(name) if name == "http") {
                return self.emit_http_call(field, args);
            }
            if matches!(target.as_ref(), Expr::Name(name) if name == "log") {
                return self.emit_log_call(field, args);
            }
            if field == "get" && self.uses_sql && !type_args.is_empty() {
                let receiver = self.emit_expr(target)?;
                let index = args
                    .first()
                    .ok_or_else(|| BackendError::unsupported("row.get without an index"))?;
                let index = self.emit_expr(index)?;
                let suffix = request_param_suffix(&type_args[0])?;
                return Ok(format!("keelSQLRowGet{suffix}({receiver}, {index})"));
            }
            if matches!(field.as_str(), "path_param" | "query_param") && !type_args.is_empty() {
                let receiver = self.emit_expr(target)?;
                let name = args.first().ok_or_else(|| {
                    BackendError::unsupported(format!("{field} without a name argument"))
                })?;
                let name = self.emit_expr(name)?;
                let suffix = request_param_suffix(&type_args[0])?;
                let func = if field == "path_param" {
                    "keelPathParam"
                } else {
                    "keelQueryParam"
                };
                return Ok(format!("{func}{suffix}({receiver}, {name})"));
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
        let mut final_args = final_args;
        // KDR-0036: fill omitted trailing arguments with the callee's declared
        // defaults. ponytail: defaults bypass box_for_slot — add when a default
        // ever flows into an erased generic slot (the M6 surface has none).
        if let Some(defaults) = self.callee_param_defaults(callee) {
            for default in defaults.into_iter().skip(args.len()).flatten() {
                let emitted = self.emit_expr(&default)?;
                final_args.push(emitted);
            }
        }
        let callee = self.emit_expr(callee)?;
        if self.uses_concurrency && is_user_function {
            final_args.insert(0, "__keel_ctx".to_string());
        }
        Ok(format!("{callee}({})", final_args.join(", ")))
    }

    /// Declared parameter defaults for a directly-named user function, in
    /// declaration order, used to fill omitted trailing arguments (KDR-0036).
    fn callee_param_defaults(&self, callee: &Expr) -> Option<Vec<Option<Expr>>> {
        let Expr::Name(name) = callee else {
            return None;
        };
        self.module.items.iter().find_map(|item| match item {
            Item::Function(function) if function.name == *name => Some(
                function
                    .params
                    .iter()
                    .map(|param| param.default.clone())
                    .collect(),
            ),
            _ => None,
        })
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

    fn register_config_type(&mut self, ty: &TypeInfo) {
        if !self.config_types.contains(ty) {
            self.config_types.push(ty.clone());
        }
    }

    /// Generate one `keelConfigLoad_<Struct>()` per config struct that
    /// `config.load<T>()` was called with (KDR-0030). Each reads its fields
    /// from the environment, keyed by `FIELD.uppercase()`, applying defaults,
    /// `Option` semantics, and the spec §15.31 parse table.
    fn emit_config_loaders(&mut self) -> Result<(), BackendError> {
        let types = self.config_types.clone();
        for ty in &types {
            self.emit_config_loader(ty)?;
            self.line("")?;
        }
        Ok(())
    }

    fn emit_config_loader(&mut self, ty: &TypeInfo) -> Result<(), BackendError> {
        let TypeInfo::Named(name) = ty else {
            return Err(BackendError::unsupported(
                "config.load on a non-struct type",
            ));
        };
        let Some(info) = self.structs.iter().find(|s| s.name == *name).cloned() else {
            return Err(BackendError::unsupported(format!(
                "config.load on unknown struct `{name}`"
            )));
        };
        self.line_fmt(format_args!(
            "func keelConfigLoad_{}() KeelEnum {{",
            json_type_name(ty)
        ))?;
        self.indent += 1;
        self.line_fmt(format_args!("var result {name}"))?;
        for field in &info.fields {
            self.emit_config_field(name, field)?;
        }
        self.line("return Ok(result)")?;
        self.indent -= 1;
        self.line("}")?;
        Ok(())
    }

    fn emit_config_field(&mut self, struct_name: &str, field: &Field) -> Result<(), BackendError> {
        let env = field.name.to_uppercase();
        let slot = format!("result.{}", field.name);
        // The else-branch when the env var is absent: a declared default, else
        // the type-appropriate "missing" error.
        let missing = if let Some(default) = &field.default {
            let value = self.emit_expr(default)?;
            format!("{slot} = {value}")
        } else if matches!(&field.ty, TypeInfo::Named(n) if n == "Secret") {
            format!("return Err(keelConfigMissingSecret({:?}))", field.name)
        } else {
            format!("return Err(keelConfigMissingEnvVar({:?}))", field.name)
        };

        if let TypeInfo::Generic { name, args } = &field.ty {
            if name == "Option" {
                let inner = args.first().cloned().unwrap_or(TypeInfo::Unknown);
                self.line_fmt(format_args!(
                    "if v, ok := os.LookupEnv({env:?}); !ok || v == \"\" {{"
                ))?;
                self.indent += 1;
                self.line_fmt(format_args!("{slot} = None"))?;
                self.indent -= 1;
                self.line("} else {")?;
                self.indent += 1;
                let parsed = self.emit_config_parse(&inner, &field.name)?;
                self.line_fmt(format_args!("{slot} = Some({parsed})"))?;
                self.indent -= 1;
                self.line("}")?;
                return Ok(());
            }
        }

        self.line_fmt(format_args!("if v, ok := os.LookupEnv({env:?}); ok {{"))?;
        self.indent += 1;
        let parsed = self.emit_config_parse(&field.ty, &field.name)?;
        self.line_fmt(format_args!("{slot} = {parsed}"))?;
        self.indent -= 1;
        self.line("} else {")?;
        self.indent += 1;
        self.line(&missing)?;
        self.indent -= 1;
        self.line("}")?;
        let _ = struct_name;
        Ok(())
    }

    /// Emit code that parses the in-scope Go string `v` into `ty`, returning a
    /// `ParseError` on failure. Leaves the parsed value as the final expression
    /// assigned by the caller. Assumes `v` is bound in the current Go scope.
    fn emit_config_parse(&mut self, ty: &TypeInfo, field: &str) -> Result<String, BackendError> {
        match ty {
            TypeInfo::String => Ok("v".to_string()),
            TypeInfo::Named(n) if n == "Secret" => Ok("keelConfigSecret{value: v}".to_string()),
            TypeInfo::Int => {
                self.line("n, err := strconv.ParseInt(v, 10, 64)")?;
                self.line_fmt(format_args!(
                    "if err != nil {{ return Err(keelConfigParseError({field:?}, \"Int\", err.Error())) }}"
                ))?;
                Ok("n".to_string())
            }
            TypeInfo::Float => {
                self.line("f, err := strconv.ParseFloat(v, 64)")?;
                self.line_fmt(format_args!(
                    "if err != nil {{ return Err(keelConfigParseError({field:?}, \"Float\", err.Error())) }}"
                ))?;
                Ok("f".to_string())
            }
            TypeInfo::Bool => {
                self.line("b, ok := keelConfigBool(v)")?;
                self.line_fmt(format_args!(
                    "if !ok {{ return Err(keelConfigParseError({field:?}, \"Bool\", \"not a boolean: \"+v)) }}"
                ))?;
                Ok("b".to_string())
            }
            other => Err(BackendError::unsupported(format!(
                "config field type `{other}`"
            ))),
        }
    }

    fn emit_struct_from_rows(&mut self) -> Result<(), BackendError> {
        if !self.uses_sql {
            return Ok(());
        }
        for info in self.structs.clone() {
            // Only column-mappable structs get a from_row; structs with Option,
            // Secret, or nested fields are not row-shaped (and never derived).
            if info.fields.iter().all(|f| Self::is_row_mappable(&f.ty)) {
                self.emit_struct_from_row(&info)?;
                self.line("")?;
            }
        }
        Ok(())
    }

    fn is_row_mappable(ty: &TypeInfo) -> bool {
        matches!(
            ty,
            TypeInfo::String | TypeInfo::Int | TypeInfo::Bool | TypeInfo::Float
        ) || matches!(ty, TypeInfo::Named(n) if n == "Uuid" || n == "Email" || n == "Timestamp")
    }

    fn emit_struct_from_row(&mut self, info: &StructInfo) -> Result<(), BackendError> {
        self.line_fmt(format_args!(
            "func keelFromRow_{}(row keelSQLRow) KeelEnum {{",
            info.name
        ))?;
        self.indent += 1;
        self.line_fmt(format_args!("var result {}", info.name))?;
        for (index, field) in info.fields.iter().enumerate() {
            let getter = self.row_field_getter(&field.ty);
            self.line_fmt(format_args!(
                "result.{} = {}(row, int64({}))",
                field.name, getter, index
            ))?;
        }
        self.line("return Ok(result)")?;
        self.indent -= 1;
        self.line("}")?;
        Ok(())
    }

    fn row_field_getter(&self, ty: &TypeInfo) -> &'static str {
        match ty {
            TypeInfo::String => "keelSQLRowGetString",
            TypeInfo::Int => "keelSQLRowGetInt",
            TypeInfo::Bool => "keelSQLRowGetBool",
            TypeInfo::Float => "keelSQLRowGetFloat",
            TypeInfo::Named(n) if n == "Uuid" => "keelSQLRowGetString",
            TypeInfo::Named(n) if n == "Email" => "keelSQLRowGetString",
            TypeInfo::Named(n) if n == "Timestamp" => "keelSQLRowGetTimestamp",
            _ => "keelSQLRowGetString",
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
        let (conds, bindings) = self.pattern_match_parts(temp, &arm.pattern);
        let cond = join_conditions(&conds);
        self.line_fmt(format_args!("if {cond} {{"))?;
        self.indent += 1;
        for (name, value) in bindings {
            self.line_fmt(format_args!("{name} := {value}"))?;
            self.line_fmt(format_args!("_ = {name}"))?;
        }
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

    /// Recursively lower a pattern into Go boolean conditions (tag checks and
    /// typed-binding membership tests) and `name := accessor` bindings, both
    /// keyed off `access` (a Go expression evaluating to the matched value).
    fn pattern_match_parts(
        &self,
        access: &str,
        pattern: &Pattern,
    ) -> (Vec<String>, Vec<(String, String)>) {
        match pattern {
            Pattern::Wildcard | Pattern::Unit => (Vec::new(), Vec::new()),
            Pattern::Name {
                name,
                is_binding: true,
                type_test,
                ..
            } => {
                let conds = type_test
                    .as_ref()
                    .map(|tags| vec![tag_membership(access, tags)])
                    .unwrap_or_default();
                (conds, vec![(name.clone(), access.to_string())])
            }
            Pattern::Name {
                name,
                args,
                payload_types,
                is_binding: false,
                ..
            } => {
                let mut conds = vec![format!("{access}.tag == {name:?}")];
                let mut bindings = Vec::new();
                for (index, arg) in args.iter().enumerate() {
                    let ty = payload_types
                        .get(index)
                        .cloned()
                        .unwrap_or(TypeInfo::Unknown);
                    let sub_access = if ty == TypeInfo::Unit {
                        "struct{}{}".to_string()
                    } else {
                        format!("{access}.values[{index}].({})", self.go_type(&ty))
                    };
                    let (sub_conds, sub_binds) = self.pattern_match_parts(&sub_access, arg);
                    conds.extend(sub_conds);
                    bindings.extend(sub_binds);
                }
                (conds, bindings)
            }
        }
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
        let (conds, bindings) = self.pattern_match_parts(temp, &arm.pattern);
        write!(out, "if {} {{ ", join_conditions(&conds))?;
        for (name, value) in bindings {
            write!(out, "{name} := {value}; _ = {name}; ")?;
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
                let body = match args.first() {
                    // Bodies may arrive as `any` (catch results, path params);
                    // keelString bridges both the `string` and `any` cases.
                    Some(arg) => format!("keelString({})", self.emit_expr(arg)?),
                    None => "\"\"".to_string(),
                };
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
        let mut emitted = Vec::new();
        for arg in args {
            emitted.push(self.emit_expr(arg)?);
        }
        let go_func = match method {
            "info" => "keelLogInfo",
            "warn" => "keelLogWarn",
            "error" => "keelLogError",
            _ => return Err(BackendError::unsupported(format!("log.{method}"))),
        };
        Ok(format!("{go_func}({})", emitted.join(", ")))
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
                StringPart::Expr { expr, ty } => {
                    let emitted = self.emit_expr(expr)?;
                    if *ty == TypeInfo::Char {
                        args.push(format!("string({emitted})"));
                    } else if *ty == TypeInfo::Named("Timestamp".to_string()) {
                        args.push(format!("keelTimestampFormat({emitted})"));
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

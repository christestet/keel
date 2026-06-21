use keelc_kir::{Block, Expr, Field, FunctionDecl, Item, Method, Module, RouteHandler, Stmt};
use keelc_types::TypeInfo;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructInfo {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceInfo {
    pub name: String,
    pub methods: Vec<Method>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImplInfo {
    pub interface_name: String,
    pub type_name: String,
    pub methods: Vec<FunctionDecl>,
}

pub fn collect_structs(module: &Module) -> Vec<StructInfo> {
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

pub fn collect_enum_variant_names(module: &Module) -> Vec<String> {
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

pub fn collect_interfaces(module: &Module) -> Vec<InterfaceInfo> {
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

pub fn collect_impls(module: &Module) -> Vec<ImplInfo> {
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

fn visit_block(block: &Block, in_struct_default: bool, visit: &mut impl FnMut(&Expr, bool)) {
    for stmt in &block.statements {
        match stmt {
            Stmt::Let { value, .. }
            | Stmt::Assign { value, .. }
            | Stmt::Expr(value)
            | Stmt::Return {
                value: Some(value), ..
            }
            | Stmt::Assert { value, .. } => visit_expr(value, in_struct_default, visit),
            Stmt::Var { .. } | Stmt::Return { value: None } | Stmt::Break | Stmt::Continue => {}
        }
    }
}

fn visit_expr(expr: &Expr, in_struct_default: bool, visit: &mut impl FnMut(&Expr, bool)) {
    visit(expr, in_struct_default);
    match expr {
        Expr::Int(_)
        | Expr::Float(_)
        | Expr::String(_)
        | Expr::Char(_)
        | Expr::Bool(_)
        | Expr::Unit
        | Expr::Name(_)
        | Expr::Return { value: None } => {}
        Expr::Unary { expr, .. }
        | Expr::Field { target: expr, .. }
        | Expr::Payload { value: expr, .. } => visit_expr(expr, in_struct_default, visit),
        Expr::Binary { left, right, .. } => {
            visit_expr(left, in_struct_default, visit);
            visit_expr(right, in_struct_default, visit);
        }
        Expr::Call { callee, args, .. } => {
            visit_expr(callee, in_struct_default, visit);
            for arg in args {
                visit_expr(arg, in_struct_default, visit);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            visit_expr(receiver, in_struct_default, visit);
            for arg in args {
                visit_expr(arg, in_struct_default, visit);
            }
        }
        Expr::StructLiteral { fields, .. } => {
            for (_, value) in fields {
                visit_expr(value, in_struct_default, visit);
            }
        }
        Expr::If {
            condition,
            then_block,
            else_block,
            ..
        } => {
            visit_expr(condition, in_struct_default, visit);
            visit_block(then_block, in_struct_default, visit);
            visit_block(else_block, in_struct_default, visit);
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            visit_expr(scrutinee, in_struct_default, visit);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    visit_expr(guard, in_struct_default, visit);
                }
                visit_expr(&arm.value, in_struct_default, visit);
            }
        }
        Expr::While { condition, body } => {
            visit_expr(condition, in_struct_default, visit);
            visit_block(body, in_struct_default, visit);
        }
        Expr::Scope { deadline, body, .. } => {
            if let Some(deadline) = deadline {
                visit_expr(deadline, in_struct_default, visit);
            }
            visit_block(body, in_struct_default, visit);
        }
        Expr::Spawn { expr, .. } => visit_expr(expr, in_struct_default, visit),
        Expr::Block(block) => visit_block(block, in_struct_default, visit),
        Expr::Router { routes, .. } => {
            for route in routes {
                if let RouteHandler::Closure { body, .. } = &route.handler {
                    visit_expr(body, in_struct_default, visit);
                }
            }
        }
        Expr::Return {
            value: Some(value), ..
        } => visit_expr(value, in_struct_default, visit),
    }
}

fn visit_module(module: &Module, visit: &mut impl FnMut(&Expr, bool)) {
    for item in &module.items {
        match item {
            Item::Struct(decl) => {
                for field in &decl.fields {
                    if let Some(default) = &field.default {
                        visit_expr(default, true, visit);
                    }
                }
            }
            Item::Function(decl) => visit_block(&decl.body, false, visit),
            Item::Impl(decl) => {
                for method in &decl.methods {
                    visit_block(&method.body, false, visit);
                }
            }
            Item::Test(decl) => visit_block(&decl.body, false, visit),
            Item::Enum(_) | Item::Interface(_) => {}
        }
    }
}

#[derive(Default)]
pub struct ModuleUsage {
    pub concurrency: bool,
    pub json: bool,
    pub http: bool,
    pub http_serve: bool,
    pub log: bool,
    pub sql: bool,
    pub config: bool,
    pub uuid_new: bool,
    pub timestamp_now: bool,
    pub config_types: Vec<TypeInfo>,
}

pub fn collect_usage(module: &Module) -> ModuleUsage {
    let mut usage = ModuleUsage::default();
    visit_module(module, &mut |expr, in_struct_default| match expr {
        Expr::Scope { .. } | Expr::Spawn { .. } => usage.concurrency = true,
        Expr::Call {
            callee, type_args, ..
        } => {
            if matches!(callee.as_ref(), Expr::Name(name) if name == "check_cancel") {
                usage.concurrency = true;
            }
            let Expr::Field { target, field, .. } = callee.as_ref() else {
                return;
            };
            let Expr::Name(name) = target.as_ref() else {
                return;
            };
            match (name.as_str(), field.as_str()) {
                ("json", "parse") => usage.json = true,
                ("http", method) if !in_struct_default => {
                    usage.http = true;
                    usage.http_serve |= method == "serve";
                }
                ("config", "load") if !in_struct_default => {
                    usage.config = true;
                    if let Some(ty) = type_args.first() {
                        if !usage.config_types.contains(ty) {
                            usage.config_types.push(ty.clone());
                        }
                    }
                }
                ("log", _) if !in_struct_default => usage.log = true,
                _ => {}
            }
        }
        Expr::MethodCall {
            receiver, method, ..
        } => {
            let Expr::Name(name) = receiver.as_ref() else {
                return;
            };
            match name.as_str() {
                "time" => usage.concurrency = true,
                "json" if method == "write" => usage.json = true,
                "Uuid" if method == "new" => usage.uuid_new = true,
                "Timestamp" if method == "now" => usage.timestamp_now = true,
                "http" if !in_struct_default => {
                    usage.http = true;
                    usage.http_serve |= method == "serve";
                }
                "sql" if method == "connect" && !in_struct_default => usage.sql = true,
                "log" if !in_struct_default => usage.log = true,
                _ => {}
            }
        }
        _ => {}
    });
    usage
}

pub fn expr_ty(expr: &Expr) -> TypeInfo {
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
        | Expr::Payload { ty, .. }
        | Expr::Scope { ty, .. }
        | Expr::Router { ty, .. }
        | Expr::Spawn { ty, .. } => ty.clone(),
        Expr::While { .. } | Expr::Return { .. } => TypeInfo::Unit,
        Expr::Block(block) => block.ty.clone(),
    }
}

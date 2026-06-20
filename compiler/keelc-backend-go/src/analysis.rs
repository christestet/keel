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

pub fn any_expr_in_block(block: &Block, pred: &impl Fn(&Expr) -> bool) -> bool {
    block.statements.iter().any(|stmt| match stmt {
        Stmt::Let { value, .. }
        | Stmt::Assign { value, .. }
        | Stmt::Expr(value)
        | Stmt::Return {
            value: Some(value), ..
        }
        | Stmt::Assert { value, .. } => any_in_expr(value, pred),
        Stmt::Var { .. } | Stmt::Return { value: None } | Stmt::Break | Stmt::Continue => false,
    })
}

pub fn any_in_expr(expr: &Expr, pred: &impl Fn(&Expr) -> bool) -> bool {
    if pred(expr) {
        return true;
    }
    match expr {
        Expr::Int(_)
        | Expr::Float(_)
        | Expr::String(_)
        | Expr::Char(_)
        | Expr::Bool(_)
        | Expr::Unit
        | Expr::Name(_)
        | Expr::Return { value: None } => false,
        Expr::Unary { expr, .. }
        | Expr::Field { target: expr, .. }
        | Expr::Payload { value: expr, .. } => any_in_expr(expr, pred),
        Expr::Binary { left, right, .. } => any_in_expr(left, pred) || any_in_expr(right, pred),
        Expr::Call { callee, args, .. } => {
            any_in_expr(callee, pred) || args.iter().any(|a| any_in_expr(a, pred))
        }
        Expr::MethodCall { receiver, args, .. } => {
            any_in_expr(receiver, pred) || args.iter().any(|a| any_in_expr(a, pred))
        }
        Expr::StructLiteral { fields, .. } => fields.iter().any(|(_, v)| any_in_expr(v, pred)),
        Expr::If {
            condition,
            then_block,
            else_block,
            ..
        } => {
            any_in_expr(condition, pred)
                || any_expr_in_block(then_block, pred)
                || any_expr_in_block(else_block, pred)
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            any_in_expr(scrutinee, pred)
                || arms.iter().any(|arm| {
                    arm.guard.as_ref().is_some_and(|g| any_in_expr(g, pred))
                        || any_in_expr(&arm.value, pred)
                })
        }
        Expr::While { condition, body } => {
            any_in_expr(condition, pred) || any_expr_in_block(body, pred)
        }
        Expr::Scope { deadline, body, .. } => {
            deadline.as_deref().is_some_and(|d| any_in_expr(d, pred))
                || any_expr_in_block(body, pred)
        }
        Expr::Spawn { expr, .. } => any_in_expr(expr, pred),
        Expr::Block(block) => any_expr_in_block(block, pred),
        Expr::Router { routes, .. } => routes.iter().any(|route| match &route.handler {
            RouteHandler::Closure { body, .. } => any_in_expr(body, pred),
            RouteHandler::Named(_) => false,
        }),
        Expr::Return {
            value: Some(value), ..
        } => any_in_expr(value, pred),
    }
}

pub fn any_in_module(module: &Module, check_structs: bool, pred: &impl Fn(&Expr) -> bool) -> bool {
    module.items.iter().any(|item| match item {
        Item::Struct(decl) if check_structs => decl
            .fields
            .iter()
            .any(|field| field.default.as_ref().is_some_and(|e| any_in_expr(e, pred))),
        Item::Function(decl) => any_expr_in_block(&decl.body, pred),
        Item::Impl(decl) => decl
            .methods
            .iter()
            .any(|method| any_expr_in_block(&method.body, pred)),
        Item::Test(decl) => any_expr_in_block(&decl.body, pred),
        Item::Enum(_) | Item::Interface(_) | Item::Struct(_) => false,
    })
}

pub fn module_uses_concurrency(module: &Module) -> bool {
    any_in_module(module, true, &|expr| {
        matches!(expr, Expr::Scope { .. } | Expr::Spawn { .. })
            || matches!(expr, Expr::Call { callee, .. }
                if matches!(callee.as_ref(), Expr::Name(name) if name == "check_cancel"))
            || matches!(expr, Expr::MethodCall { receiver, .. }
                if matches!(receiver.as_ref(), Expr::Name(name) if name == "time"))
    })
}

pub fn module_uses_json(module: &Module) -> bool {
    any_in_module(module, true, &|expr| match expr {
        Expr::Call { callee, .. } => matches!(callee.as_ref(),
            Expr::Field { target, field, .. }
                if field == "parse" && matches!(target.as_ref(), Expr::Name(name) if name == "json")),
        Expr::MethodCall {
            receiver, method, ..
        } => method == "write" && matches!(receiver.as_ref(), Expr::Name(name) if name == "json"),
        _ => false,
    })
}

pub fn module_uses_http_serve(module: &Module) -> bool {
    any_in_module(module, false, &|expr| match expr {
        Expr::Call { callee, .. } => matches!(callee.as_ref(),
            Expr::Field { target, field, .. }
                if field == "serve" && matches!(target.as_ref(), Expr::Name(name) if name == "http")),
        Expr::MethodCall {
            receiver, method, ..
        } => method == "serve" && matches!(receiver.as_ref(), Expr::Name(name) if name == "http"),
        _ => false,
    })
}

pub fn module_uses_http(module: &Module) -> bool {
    any_in_module(module, false, &|expr| match expr {
        Expr::Call { callee, .. } => matches!(callee.as_ref(),
            Expr::Field { target, field, .. }
                if matches!(target.as_ref(), Expr::Name(name) if name == "http")),
        Expr::MethodCall { receiver, .. } => {
            matches!(receiver.as_ref(), Expr::Name(name) if name == "http")
        }
        _ => false,
    })
}

pub fn module_uses_sql(module: &Module) -> bool {
    // Every `std.sql` program connects first; that single call is the trigger.
    any_in_module(module, false, &|expr| {
        matches!(expr,
            Expr::MethodCall { receiver, method, .. }
                if method == "connect"
                    && matches!(receiver.as_ref(), Expr::Name(name) if name == "sql"))
    })
}

pub fn module_uses_log(module: &Module) -> bool {
    any_in_module(module, false, &|expr| match expr {
        Expr::Call { callee, .. } => matches!(callee.as_ref(),
            Expr::Field { target, field, .. }
                if matches!(target.as_ref(), Expr::Name(name) if name == "log")),
        Expr::MethodCall { receiver, .. } => {
            matches!(receiver.as_ref(), Expr::Name(name) if name == "log")
        }
        _ => false,
    })
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

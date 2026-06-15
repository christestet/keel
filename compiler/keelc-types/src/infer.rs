//! Shared, diagnostic-free type-inference helpers for the Keel pipeline.
//!
//! [`TypeContext`] captures the module-level symbol tables and local binding
//! scopes needed to infer expression types.  It is intentionally agnostic to
//! diagnostics so that both `keelc-resolve` (which adds diagnostics) and
//! `keelc-kir` lowering (which needs typed KIR) can share the same inference
//! logic.

use crate::{merge_types, substitute_type_params, type_param_bounds, TypeInfo};
use keelc_ast::{BinaryOp, Block, Expr, Item, MatchArm, Module, Stmt, UnaryOp};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructInfo {
    pub name: String,
    pub fields: Vec<StructFieldInfo>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructFieldInfo {
    pub name: String,
    pub ty: TypeInfo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionInfo {
    pub name: String,
    pub params: Vec<TypeInfo>,
    pub return_type: TypeInfo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumInfo {
    pub name: String,
    pub variants: Vec<VariantInfo>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantInfo {
    pub name: String,
    pub fields: Vec<VariantFieldInfo>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantFieldInfo {
    pub name: String,
    pub ty: TypeInfo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceInfo {
    pub name: String,
    pub methods: Vec<MethodInfo>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImplInfo {
    pub interface_name: String,
    pub type_name: String,
    pub methods: Vec<MethodInfo>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MethodInfo {
    pub name: String,
    pub params: Vec<TypeInfo>,
    pub return_type: TypeInfo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypedBinding {
    pub name: String,
    pub ty: TypeInfo,
}

/// Type-inference context shared by resolution and KIR lowering.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeContext {
    functions: Vec<FunctionInfo>,
    enums: Vec<EnumInfo>,
    structs: Vec<StructInfo>,
    interfaces: Vec<InterfaceInfo>,
    impls: Vec<ImplInfo>,
    scopes: Vec<Vec<TypedBinding>>,
    current_return_type: Option<TypeInfo>,
}

impl TypeContext {
    #[must_use]
    pub fn new(module: &Module) -> Self {
        let interfaces = collect_interfaces(module);
        let interface_names: Vec<String> =
            interfaces.iter().map(|info| info.name.clone()).collect();
        Self {
            functions: collect_functions(module, &interface_names),
            enums: collect_enums(module),
            structs: collect_structs(module),
            interfaces,
            impls: collect_impls(module, &interface_names),
            scopes: Vec::new(),
            current_return_type: None,
        }
    }

    pub fn set_current_return_type(&mut self, return_type: TypeInfo) {
        self.current_return_type = Some(return_type);
    }

    pub fn clear_current_return_type(&mut self) {
        self.current_return_type = None;
    }

    #[must_use]
    pub fn current_return_type(&self) -> Option<&TypeInfo> {
        self.current_return_type.as_ref()
    }

    #[must_use]
    pub fn functions(&self) -> &[FunctionInfo] {
        &self.functions
    }

    #[must_use]
    pub fn enums(&self) -> &[EnumInfo] {
        &self.enums
    }

    #[must_use]
    pub fn interfaces(&self) -> &[InterfaceInfo] {
        &self.interfaces
    }

    #[must_use]
    pub fn impls(&self) -> &[ImplInfo] {
        &self.impls
    }

    #[must_use]
    pub fn resolve_type(&self, ty: &TypeInfo) -> TypeInfo {
        self.resolve_type_inner(ty.clone())
    }

    #[must_use]
    pub fn infer_expr(&self, expr: &Expr) -> TypeInfo {
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
            Expr::Unary { op, expr, .. } => self.infer_unary(*op, expr),
            Expr::Binary {
                left, op, right, ..
            } => self.infer_binary(left, *op, right),
            Expr::Call { callee, args, .. } => self.infer_call(callee, args),
            Expr::Field { target, field, .. } => self
                .field_type(&self.infer_expr(target), &field.value)
                .unwrap_or(TypeInfo::Unknown),
            Expr::MethodCall {
                receiver,
                method,
                args,
                ..
            } => self.infer_method_call(receiver, &method.value, args),
            Expr::StructLiteral { name, .. } => TypeInfo::Named(name.value.clone()),
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
            Expr::Match { arms, .. } => self.infer_match_result(arms),
            Expr::While { .. } => TypeInfo::Unit,
            Expr::Block(block) => self.infer_block_type(block),
            Expr::Question { expr, .. } => self.infer_question(expr),
            Expr::Catch { expr, .. } => self
                .infer_expr(expr)
                .result_parts()
                .map(|(ok, _)| ok)
                .cloned()
                .unwrap_or(TypeInfo::Unknown),
            Expr::Return { .. } => TypeInfo::Unit,
        }
    }

    #[must_use]
    pub fn infer_unary(&self, op: UnaryOp, expr: &Expr) -> TypeInfo {
        let operand_type = self.infer_expr(expr);
        match op {
            UnaryOp::Negate if operand_type.is_numeric() => operand_type,
            UnaryOp::Not => TypeInfo::Bool,
            UnaryOp::Negate => TypeInfo::Unknown,
        }
    }

    #[must_use]
    pub fn infer_binary(&self, left: &Expr, op: BinaryOp, right: &Expr) -> TypeInfo {
        let left_type = self.infer_expr(left);
        let right_type = self.infer_expr(right);
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

    #[must_use]
    pub fn infer_call(&self, callee: &Expr, args: &[Expr]) -> TypeInfo {
        let arg_types: Vec<TypeInfo> = args.iter().map(|arg| self.infer_expr(arg)).collect();
        match callee {
            Expr::Name(name) if name.value == "print" => TypeInfo::Unit,
            Expr::Name(name) if name.value == "checked_div" || name.value == "checked_rem" => {
                TypeInfo::generic("Option", vec![TypeInfo::Int])
            }
            Expr::Name(name) if name.value == "Some" => TypeInfo::generic(
                "Option",
                vec![arg_types.first().cloned().unwrap_or(TypeInfo::Unknown)],
            ),
            Expr::Name(name) if name.value == "Ok" => TypeInfo::generic(
                "Result",
                vec![
                    arg_types.first().cloned().unwrap_or(TypeInfo::Unknown),
                    TypeInfo::Unknown,
                ],
            ),
            Expr::Name(name) if name.value == "Err" => TypeInfo::generic(
                "Result",
                vec![
                    TypeInfo::Unknown,
                    arg_types.first().cloned().unwrap_or(TypeInfo::Unknown),
                ],
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

    #[must_use]
    pub fn infer_method_call(&self, receiver: &Expr, method: &str, args: &[Expr]) -> TypeInfo {
        if matches!(receiver, Expr::Name(name) if name.value == "Float") && method == "from" {
            return TypeInfo::Float;
        }
        let receiver_type = self.infer_expr(receiver);
        for arg in args {
            let _ = self.infer_expr(arg);
        }
        let method_info = match &receiver_type {
            TypeInfo::Interface(name) | TypeInfo::TypeParam { bound: name, .. } => self
                .interface_info(name)
                .and_then(|interface| interface.methods.iter().find(|m| m.name == method).cloned()),
            TypeInfo::Named(type_name) => self
                .impls
                .iter()
                .filter(|info| info.type_name == *type_name)
                .flat_map(|info| info.methods.iter())
                .find(|m| m.name == method)
                .cloned(),
            _ => None,
        };
        method_info
            .map(|info| info.return_type)
            .unwrap_or(TypeInfo::Unknown)
    }

    #[must_use]
    pub fn infer_block_type(&self, block: &Block) -> TypeInfo {
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

    #[must_use]
    pub fn infer_match_result(&self, arms: &[MatchArm]) -> TypeInfo {
        let mut result = TypeInfo::Unknown;
        for arm in arms {
            let arm_type = self.infer_expr(&arm.value);
            result = merge_types(&result, &arm_type);
        }
        result
    }

    #[must_use]
    pub fn infer_question(&self, expr: &Expr) -> TypeInfo {
        let expr_type = self.infer_expr(expr);
        match &expr_type {
            TypeInfo::Generic { name, args } if name == "Result" && args.len() == 2 => {
                let (Some(success_type), Some(error_type)) = (args.first(), args.get(1)) else {
                    return TypeInfo::Unknown;
                };
                let can_absorb = self
                    .current_return_type
                    .as_ref()
                    .and_then(|ty| ty.result_parts())
                    .is_some_and(|(_, return_error)| type_absorbs(return_error, error_type));
                if can_absorb {
                    success_type.clone()
                } else {
                    TypeInfo::Unknown
                }
            }
            TypeInfo::Generic { name, args } if name == "Option" && args.len() == 1 => {
                let Some(success_type) = args.first() else {
                    return TypeInfo::Unknown;
                };
                let can_absorb = self
                    .current_return_type
                    .as_ref()
                    .and_then(|ty| ty.option_inner())
                    .is_some();
                if can_absorb {
                    success_type.clone()
                } else {
                    TypeInfo::Unknown
                }
            }
            _ => TypeInfo::Unknown,
        }
    }

    #[must_use]
    pub fn pattern_payload_types(
        &self,
        scrutinee_ty: &TypeInfo,
        pattern_name: &str,
    ) -> Vec<TypeInfo> {
        if pattern_name == "Some" {
            if let Some(inner) = scrutinee_ty.option_inner() {
                return vec![inner.clone()];
            }
        }
        if pattern_name == "Ok" || pattern_name == "Err" {
            if let Some((ok, err)) = scrutinee_ty.result_parts() {
                return vec![if pattern_name == "Ok" {
                    ok.clone()
                } else {
                    err.clone()
                }];
            }
        }
        if let TypeInfo::Named(name) = scrutinee_ty {
            return self
                .enums
                .iter()
                .find(|info| info.name == *name)
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
                .unwrap_or_default();
        }
        Vec::new()
    }

    #[must_use]
    pub fn builtin_value_type(&self, name: &str) -> Option<TypeInfo> {
        match name {
            "None" => Some(TypeInfo::generic("Option", vec![TypeInfo::Unknown])),
            _ => None,
        }
    }

    #[must_use]
    pub fn enum_variant_type(&self, variant_name: &str) -> Option<TypeInfo> {
        self.enums
            .iter()
            .find(|info| {
                info.variants
                    .iter()
                    .any(|variant| variant.name == variant_name)
            })
            .map(|info| TypeInfo::Named(info.name.clone()))
    }

    #[must_use]
    pub fn function_return_type(&self, name: &str) -> Option<TypeInfo> {
        self.functions
            .iter()
            .find(|function| function.name == name)
            .map(|function| function.return_type.clone())
    }

    #[must_use]
    pub fn field_type(&self, target_ty: &TypeInfo, field_name: &str) -> Option<TypeInfo> {
        let TypeInfo::Named(name) = target_ty else {
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

    #[must_use]
    pub fn interface_info(&self, name: &str) -> Option<&InterfaceInfo> {
        self.interfaces.iter().find(|info| info.name == name)
    }

    #[must_use]
    pub fn function_info(&self, name: &str) -> Option<&FunctionInfo> {
        self.functions.iter().find(|function| function.name == name)
    }

    #[must_use]
    pub fn exhaustive_variants(&self, ty: &TypeInfo) -> Option<Vec<String>> {
        match ty {
            TypeInfo::Named(name) => {
                self.enums
                    .iter()
                    .find(|info| info.name == *name)
                    .map(|info| {
                        info.variants
                            .iter()
                            .map(|variant| variant.name.clone())
                            .collect()
                    })
            }
            TypeInfo::Generic { name, .. } if name == "Option" => {
                Some(vec!["Some".to_string(), "None".to_string()])
            }
            TypeInfo::Generic { name, .. } if name == "Result" => {
                Some(vec!["Ok".to_string(), "Err".to_string()])
            }
            TypeInfo::Union(members) => {
                let mut variants = Vec::new();
                for member in members {
                    let member_variants = self.exhaustive_variants(member)?;
                    variants.extend(member_variants);
                }
                Some(variants)
            }
            _ => None,
        }
    }

    #[must_use]
    pub fn value_type(&self, name: &str) -> Option<TypeInfo> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev())
            .find(|binding| binding.name == name)
            .map(|binding| binding.ty.clone())
    }

    pub fn define_value(&mut self, name: &str, ty: TypeInfo) {
        let resolved = self.resolve_type_inner(ty);
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(TypedBinding {
                name: name.to_string(),
                ty: resolved,
            });
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    pub fn pop_scope(&mut self) {
        let _ = self.scopes.pop();
    }

    fn resolve_type_inner(&self, ty: TypeInfo) -> TypeInfo {
        match &ty {
            TypeInfo::Named(name) if self.interfaces.iter().any(|info| info.name == *name) => {
                TypeInfo::Interface(name.clone())
            }
            TypeInfo::Generic { name, args } => TypeInfo::Generic {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|arg| self.resolve_type_inner(arg.clone()))
                    .collect(),
            },
            TypeInfo::Union(members) => TypeInfo::Union(
                members
                    .iter()
                    .map(|member| self.resolve_type_inner(member.clone()))
                    .collect(),
            ),
            _ => ty,
        }
    }
}

fn collect_structs(module: &Module) -> Vec<StructInfo> {
    let mut structs = Vec::new();
    for item in &module.items {
        if let Item::Struct(decl) = item {
            let params = type_param_bounds(&decl.type_params);
            structs.push(StructInfo {
                name: decl.name.value.clone(),
                fields: decl
                    .fields
                    .iter()
                    .map(|field| StructFieldInfo {
                        name: field.name.value.clone(),
                        ty: substitute_type_params(&TypeInfo::from_ast(&field.ty), &params),
                    })
                    .collect(),
            });
        }
    }
    structs.sort_by(|left, right| left.name.cmp(&right.name));
    structs
}

fn collect_functions(module: &Module, interface_names: &[String]) -> Vec<FunctionInfo> {
    let resolve = |ty: TypeInfo| {
        if let TypeInfo::Named(name) = &ty {
            if interface_names.iter().any(|info| info == name) {
                return TypeInfo::Interface(name.clone());
            }
        }
        ty
    };
    let mut functions = Vec::new();
    for item in &module.items {
        if let Item::Function(decl) = item {
            let type_params = type_param_bounds(&decl.type_params);
            let return_type = substitute_type_params(
                &decl
                    .return_type
                    .as_ref()
                    .map_or(TypeInfo::Unit, TypeInfo::from_ast)
                    .map_type(&resolve),
                &type_params,
            );
            let params = decl
                .params
                .iter()
                .map(|param| {
                    substitute_type_params(
                        &param
                            .ty
                            .as_ref()
                            .map_or(TypeInfo::Unknown, TypeInfo::from_ast)
                            .map_type(&resolve),
                        &type_params,
                    )
                })
                .collect();
            functions.push(FunctionInfo {
                name: decl.name.value.clone(),
                params,
                return_type,
            });
        }
    }
    functions.sort_by(|left, right| left.name.cmp(&right.name));
    functions
}

fn collect_enums(module: &Module) -> Vec<EnumInfo> {
    let mut enums = Vec::new();
    for item in &module.items {
        if let Item::Enum(decl) = item {
            let mut variants: Vec<VariantInfo> = decl
                .variants
                .iter()
                .map(|variant| VariantInfo {
                    name: variant.name.value.clone(),
                    fields: variant
                        .fields
                        .iter()
                        .map(|field| VariantFieldInfo {
                            name: field.name.value.clone(),
                            ty: TypeInfo::from_ast(&field.ty),
                        })
                        .collect(),
                })
                .collect();
            variants.sort_by(|left, right| left.name.cmp(&right.name));
            enums.push(EnumInfo {
                name: decl.name.value.clone(),
                variants,
            });
        }
    }
    enums.sort_by(|left, right| left.name.cmp(&right.name));
    enums
}

fn interface_names(module: &Module) -> Vec<String> {
    let mut names = Vec::new();
    for item in &module.items {
        if let Item::Interface(decl) = item {
            names.push(decl.name.value.clone());
        }
    }
    names.sort();
    names
}

fn collect_interfaces(module: &Module) -> Vec<InterfaceInfo> {
    let names = interface_names(module);
    let mut interfaces = Vec::new();
    for item in &module.items {
        if let Item::Interface(decl) = item {
            let methods = decl
                .methods
                .iter()
                .map(|m| method_from_decl(m, &names))
                .collect();
            interfaces.push(InterfaceInfo {
                name: decl.name.value.clone(),
                methods,
            });
        }
    }
    interfaces.sort_by(|left, right| left.name.cmp(&right.name));
    interfaces
}

fn collect_impls(module: &Module, interfaces: &[String]) -> Vec<ImplInfo> {
    let names: Vec<String> = interfaces.to_vec();
    let mut impls = Vec::new();
    for item in &module.items {
        if let Item::Impl(decl) = item {
            let methods = decl
                .methods
                .iter()
                .map(|m| method_from_decl(m, &names))
                .collect();
            impls.push(ImplInfo {
                interface_name: decl.interface_name.value.clone(),
                type_name: decl.type_name.value.clone(),
                methods,
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

pub fn method_from_decl(decl: &keelc_ast::FunctionDecl, interface_names: &[String]) -> MethodInfo {
    let resolve = |ty: TypeInfo| {
        if let TypeInfo::Named(name) = &ty {
            if interface_names.iter().any(|info| info == name) {
                return TypeInfo::Interface(name.clone());
            }
        }
        ty
    };
    let params = decl
        .params
        .iter()
        .filter(|param| param.name.value != "self")
        .map(|param| {
            param
                .ty
                .as_ref()
                .map_or(TypeInfo::Unknown, TypeInfo::from_ast)
                .map_type(&resolve)
        })
        .collect();
    let return_type = decl
        .return_type
        .as_ref()
        .map_or(TypeInfo::Unit, TypeInfo::from_ast)
        .map_type(&resolve);
    MethodInfo {
        name: decl.name.value.clone(),
        params,
        return_type,
    }
}

#[must_use]
pub fn types_compatible(left: &TypeInfo, right: &TypeInfo) -> bool {
    if left == right || matches!(left, TypeInfo::Unknown) || matches!(right, TypeInfo::Unknown) {
        return true;
    }
    match (left, right) {
        (
            TypeInfo::Generic {
                name: left_name,
                args: left_args,
            },
            TypeInfo::Generic {
                name: right_name,
                args: right_args,
            },
        ) if left_name == right_name && left_args.len() == right_args.len() => left_args
            .iter()
            .zip(right_args)
            .all(|(left, right)| types_compatible(left, right)),
        (TypeInfo::Union(left_members), TypeInfo::Union(right_members)) => {
            left_members.len() == right_members.len()
                && left_members
                    .iter()
                    .zip(right_members)
                    .all(|(left, right)| types_compatible(left, right))
        }
        _ => false,
    }
}

#[must_use]
pub fn type_absorbs(target: &TypeInfo, source: &TypeInfo) -> bool {
    target == source
        || matches!(source, TypeInfo::Unknown)
        || match (target, source) {
            (TypeInfo::Union(_), TypeInfo::Union(sources)) => {
                sources.iter().all(|source| type_absorbs(target, source))
            }
            (TypeInfo::Union(targets), source) => targets.iter().any(|target| target == source),
            _ => false,
        }
}

#[must_use]
pub fn is_int_float_pair(left: &TypeInfo, right: &TypeInfo) -> bool {
    matches!(
        (left, right),
        (TypeInfo::Int, TypeInfo::Float) | (TypeInfo::Float, TypeInfo::Int)
    )
}

#[must_use]
pub fn question_success_type(ty: &TypeInfo) -> Option<TypeInfo> {
    ty.option_inner()
        .or_else(|| ty.result_parts().map(|(ok, _)| ok))
        .cloned()
}

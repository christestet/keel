//! Shared type information and symbol tables for the Keel pipeline.
//!
//! [`TypeContext`] captures the module-level symbol tables and local binding
//! scopes used by resolution, plus declarations queried during KIR lowering.

use crate::{substitute_type_params, type_param_bounds, TypeInfo};
use keelc_ast::{Item, Module};

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

/// Shared type context for resolution and KIR lowering.
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

/// Binary-search a name-sorted slice (every `collect_*` sorts by name).
/// Returns the first match, exactly what `iter().find` returned on the sorted
/// vec — but in O(log n), which is what keeps `keel check` inside its
/// KDR-0019 budget on declaration-heavy modules.
#[must_use]
pub fn find_by_name<'a, T>(items: &'a [T], name: &str, key: fn(&T) -> &str) -> Option<&'a T> {
    let idx = items.partition_point(|item| key(item) < name);
    items.get(idx).filter(|item| key(item) == name)
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
    pub fn pattern_payload_types(
        &self,
        scrutinee_ty: &TypeInfo,
        pattern_name: &str,
    ) -> Vec<TypeInfo> {
        // A union member carries the variant: try each member in turn.
        if let TypeInfo::Union(members) = scrutinee_ty {
            for member in members {
                let payloads = self.pattern_payload_types(member, pattern_name);
                if !payloads.is_empty() {
                    return payloads;
                }
            }
            return Vec::new();
        }
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
        if scrutinee_ty == &TypeInfo::Named("json.Error".to_string()) {
            return match pattern_name {
                "Syntax" => vec![TypeInfo::Int],
                "TypeMismatch" => vec![TypeInfo::String, TypeInfo::String],
                "MissingField" | "UnknownField" | "DuplicateField" | "OutOfRange" | "NonFinite" => {
                    vec![TypeInfo::String]
                }
                _ => Vec::new(),
            };
        }
        if scrutinee_ty == &TypeInfo::Named("http.Error".to_string()) {
            return match pattern_name {
                "BindFailed" => vec![TypeInfo::String],
                _ => Vec::new(),
            };
        }
        if let TypeInfo::Named(name) = scrutinee_ty {
            return find_by_name(&self.enums, name, |info| &info.name)
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
            "Cancelled" => Some(TypeInfo::Named("Cancelled".to_string())),
            "Syntax" | "TypeMismatch" | "MissingField" | "UnknownField" | "DuplicateField"
            | "OutOfRange" | "NonFinite" => Some(TypeInfo::Named("json.Error".to_string())),
            "BindFailed" => Some(TypeInfo::Named("http.Error".to_string())),
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
        self.function_info(name)
            .map(|function| function.return_type.clone())
    }

    #[must_use]
    pub fn field_type(&self, target_ty: &TypeInfo, field_name: &str) -> Option<TypeInfo> {
        if field_name == "value" {
            if let Some(inner) = task_inner(target_ty) {
                return Some(task_value_type(inner));
            }
        }
        if let TypeInfo::Named(name) = target_ty {
            if name == "http.Response" {
                return match field_name {
                    "status" => Some(TypeInfo::Int),
                    "body" => Some(TypeInfo::String),
                    _ => None,
                };
            }
            if name == "http.Request" {
                return match field_name {
                    "body" | "method" | "path" => Some(TypeInfo::String),
                    _ => None,
                };
            }
        }
        let TypeInfo::Named(name) = target_ty else {
            return None;
        };
        find_by_name(&self.structs, name, |info| &info.name)?
            .fields
            .iter()
            .find(|field| field.name == field_name)
            .map(|field| field.ty.clone())
    }

    #[must_use]
    pub fn is_struct(&self, name: &str) -> bool {
        find_by_name(&self.structs, name, |info| &info.name).is_some()
    }

    #[must_use]
    pub fn is_enum(&self, name: &str) -> bool {
        find_by_name(&self.enums, name, |info| &info.name).is_some()
    }

    #[must_use]
    pub fn interface_info(&self, name: &str) -> Option<&InterfaceInfo> {
        find_by_name(&self.interfaces, name, |info| &info.name)
    }

    #[must_use]
    pub fn function_info(&self, name: &str) -> Option<&FunctionInfo> {
        find_by_name(&self.functions, name, |function| &function.name)
    }

    #[must_use]
    pub fn exhaustive_variants(&self, ty: &TypeInfo) -> Option<Vec<String>> {
        match ty {
            TypeInfo::Named(name) if name == "Cancelled" => Some(vec!["Cancelled".to_string()]),
            TypeInfo::Named(name) if name == "json.Error" => Some(vec![
                "Syntax".to_string(),
                "TypeMismatch".to_string(),
                "MissingField".to_string(),
                "UnknownField".to_string(),
                "DuplicateField".to_string(),
                "OutOfRange".to_string(),
                "NonFinite".to_string(),
            ]),
            TypeInfo::Named(name) if name == "http.Error" => Some(vec!["BindFailed".to_string()]),
            TypeInfo::Named(name) => {
                find_by_name(&self.enums, name, |info| &info.name).map(|info| {
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

    #[must_use]
    pub fn is_json_representable(&self, ty: &TypeInfo) -> bool {
        match ty {
            TypeInfo::Int
            | TypeInfo::Float
            | TypeInfo::Bool
            | TypeInfo::String
            | TypeInfo::Char => true,
            TypeInfo::Named(name) if matches!(name.as_str(), "Uuid" | "Timestamp" | "Email") => {
                true
            }
            TypeInfo::Generic { name, args } if name == "Option" || name == "List" => {
                args.len() == 1 && self.is_json_representable(&args[0])
            }
            TypeInfo::Generic { name, args } if name == "Map" => {
                args.len() == 2
                    && args[0] == TypeInfo::String
                    && self.is_json_representable(&args[1])
            }
            TypeInfo::Named(name) => self.is_struct(name) || self.is_enum(name),
            TypeInfo::Unit
            | TypeInfo::Interface(_)
            | TypeInfo::TypeParam { .. }
            | TypeInfo::Union(_)
            | TypeInfo::Unknown
            | TypeInfo::Generic { .. } => false,
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
            TypeInfo::Named(name) if self.interface_info(name).is_some() => {
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
        // The universal `Error` (KDR-0033) is the one type that absorbs every
        // error: any propagated `E` coerces into a `Result<_, Error>` context.
        || matches!(target, TypeInfo::Named(name) if name == "Error")
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

pub fn task_inner(ty: &TypeInfo) -> Option<&TypeInfo> {
    match ty {
        TypeInfo::Generic { name, args } if name == "Task" && args.len() == 1 => args.first(),
        _ => None,
    }
}

pub fn task_value_type(inner: &TypeInfo) -> TypeInfo {
    inner
        .result_parts()
        .map_or_else(|| inner.clone(), |(ok, _)| ok.clone())
}

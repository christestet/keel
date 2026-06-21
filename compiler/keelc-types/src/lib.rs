//! Shared type definitions for the Keel compiler pipeline.
//!
//! Holds the canonical [`TypeInfo`] enum used by name resolution, type
//! checking, and backends.  Keeps the same type model in one place so that
//! adding a new type doesn't require touching multiple crates.

pub mod infer;

use keelc_ast::Type as AstType;
use std::fmt;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TypeInfo {
    Int,
    Float,
    Bool,
    String,
    Char,
    Unit,
    Named(String),
    Interface(String),
    /// A generic type parameter `T` constrained by interface `bound`.
    /// Erased to its bound interface by the backend; kept distinct during
    /// checking so method access (`K0802`) and constraint satisfaction
    /// (`K0803`) can be diagnosed separately from nominal interface values.
    TypeParam {
        name: String,
        bound: String,
    },
    Generic {
        name: String,
        args: Vec<TypeInfo>,
    },
    Union(Vec<TypeInfo>),
    Unknown,
}

impl TypeInfo {
    pub fn from_ast(ty: &AstType) -> Self {
        match ty {
            AstType::Named { name, args, .. } if args.is_empty() => match name.value.as_str() {
                "Int" => Self::Int,
                "Float" => Self::Float,
                "Bool" => Self::Bool,
                "String" => Self::String,
                "Char" => Self::Char,
                "Unit" => Self::Unit,
                _ => Self::Named(name.value.clone()),
            },
            AstType::Named { name, args, .. } => Self::Generic {
                name: name.value.clone(),
                args: args.iter().map(Self::from_ast).collect(),
            },
            AstType::Union { members, .. } => {
                Self::Union(members.iter().map(Self::from_ast).collect())
            }
        }
    }

    pub const fn is_numeric(&self) -> bool {
        matches!(self, Self::Int | Self::Float)
    }

    pub fn option_inner(&self) -> Option<&Self> {
        match self {
            Self::Generic { name, args } if name == "Option" && args.len() == 1 => args.first(),
            _ => None,
        }
    }

    pub fn generic(name: impl Into<String>, args: Vec<Self>) -> Self {
        Self::Generic {
            name: name.into(),
            args,
        }
    }

    pub fn result_parts(&self) -> Option<(&Self, &Self)> {
        match self {
            Self::Generic { name, args } if name == "Result" && args.len() == 2 => {
                Some((args.first()?, args.get(1)?))
            }
            _ => None,
        }
    }

    pub fn map_type(&self, f: &impl Fn(Self) -> Self) -> Self {
        let mapped = f(self.clone());
        match mapped {
            Self::Generic { name, args } => Self::Generic {
                name,
                args: args.iter().map(|arg| arg.map_type(f)).collect(),
            },
            Self::Union(members) => {
                Self::Union(members.iter().map(|member| member.map_type(f)).collect())
            }
            other => other,
        }
    }
}

impl fmt::Display for TypeInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Int => f.write_str("Int"),
            Self::Float => f.write_str("Float"),
            Self::Bool => f.write_str("Bool"),
            Self::String => f.write_str("String"),
            Self::Char => f.write_str("Char"),
            Self::Unit => f.write_str("Unit"),
            Self::Named(name) => f.write_str(name),
            Self::Interface(name) => f.write_str(name),
            Self::TypeParam { name, .. } => f.write_str(name),
            Self::Generic { name, args } => {
                write!(f, "{name}<")?;
                write_type_list(f, args, ", ")?;
                f.write_str(">")
            }
            Self::Union(members) => write_type_list(f, members, " | "),
            Self::Unknown => f.write_str("<unknown>"),
        }
    }
}

fn write_type_list(f: &mut fmt::Formatter<'_>, types: &[TypeInfo], separator: &str) -> fmt::Result {
    let mut first = true;
    for ty in types {
        if first {
            first = false;
        } else {
            f.write_str(separator)?;
        }
        write!(f, "{ty}")?;
    }
    Ok(())
}

/// Reduce a collected list of error types into a single optional type.
///
/// Returns `None` for an empty list, the sole type for a single error, and
/// `Some(TypeInfo::Union(errors))` when multiple distinct error types are
/// present.  Callers are responsible for deduplication — this function does
/// not sort or deduplicate.
#[must_use]
pub fn reduce_error_types(errors: Vec<TypeInfo>) -> Option<TypeInfo> {
    match errors.len() {
        0 => None,
        1 => errors.into_iter().next(),
        _ => Some(TypeInfo::Union(errors)),
    }
}

/// Extract `(name, bound-interface)` pairs from a declaration's type parameters.
/// An unbound parameter (already reported as `K0801` by the parser) yields an
/// empty bound string.
#[must_use]
pub fn type_param_bounds(type_params: &[keelc_ast::TypeParam]) -> Vec<(String, String)> {
    type_params
        .iter()
        .map(|tp| {
            (
                tp.name.value.clone(),
                tp.bound
                    .as_ref()
                    .map_or_else(String::new, |bound| bound.value.clone()),
            )
        })
        .collect()
}

/// Rewrite any `Named(T)` that matches a type-parameter name into the
/// corresponding [`TypeInfo::TypeParam`], recursing through generic and union
/// types. Used by the typechecker and KIR lowering so type parameters carry
/// their interface bound.
#[must_use]
pub fn substitute_type_params(ty: &TypeInfo, params: &[(String, String)]) -> TypeInfo {
    ty.map_type(&|inner| match &inner {
        TypeInfo::Named(name) => params
            .iter()
            .find(|(param_name, _)| param_name == name)
            .map_or(inner.clone(), |(param_name, bound)| TypeInfo::TypeParam {
                name: param_name.clone(),
                bound: bound.clone(),
            }),
        _ => inner,
    })
}

#[must_use]
pub fn merge_types(left: &TypeInfo, right: &TypeInfo) -> TypeInfo {
    if matches!(left, TypeInfo::Unknown) {
        return right.clone();
    }
    if matches!(right, TypeInfo::Unknown) || left == right {
        return left.clone();
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
        ) if left_name == right_name && left_args.len() == right_args.len() => TypeInfo::Generic {
            name: left_name.clone(),
            args: left_args
                .iter()
                .zip(right_args)
                .map(|(left, right)| merge_types(left, right))
                .collect(),
        },
        (TypeInfo::Union(left_members), TypeInfo::Union(right_members))
            if left_members.len() == right_members.len() =>
        {
            TypeInfo::Union(
                left_members
                    .iter()
                    .zip(right_members)
                    .map(|(left, right)| merge_types(left, right))
                    .collect(),
            )
        }
        _ => TypeInfo::Unknown,
    }
}

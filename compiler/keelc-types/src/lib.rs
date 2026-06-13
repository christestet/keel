//! Shared type definitions for the Keel compiler pipeline.
//!
//! Holds the canonical [`TypeInfo`] enum used by name resolution, type
//! checking, and backends.  Keeps the same type model in one place so that
//! adding a new type doesn't require touching multiple crates.

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
    Generic { name: String, args: Vec<TypeInfo> },
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

    pub fn result_parts(&self) -> Option<(&Self, &Self)> {
        match self {
            Self::Generic { name, args } if name == "Result" && args.len() == 2 => {
                Some((args.first()?, args.get(1)?))
            }
            _ => None,
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

fn write_type_list(
    f: &mut fmt::Formatter<'_>,
    types: &[TypeInfo],
    separator: &str,
) -> fmt::Result {
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

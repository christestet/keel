//! Append-only registry of stable diagnostic codes used by keelc.

use crate::Code;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryEntry {
    pub code: Code,
    pub summary: &'static str,
}

macro_rules! codes {
    ($($name:ident => $summary:expr),* $(,)?) => {
        $(pub const $name: Code = Code::new(stringify!($name));)*
        pub const ALL_CODES: &[RegistryEntry] = &[$(RegistryEntry { code: $name, summary: $summary },)*];
    };
}

codes! {
    K0001 => "unrecognized character",
    K0002 => "unterminated string literal",
    K0003 => "syntax error",
    K0004 => "malformed string interpolation",
    K0101 => "identifier casing violation",
    K0102 => "semicolon used as a statement terminator",
    K0201 => "nullish construct used",
    K0202 => "implicit numeric conversion",
    K0203 => "integer overflow rule violation",
    K0204 => "division or remainder by zero",
    K0301 => "struct construction missing required field",
    K0302 => "function signature type annotation required",
    K0303 => "assignment to immutable binding",
    K0401 => "if/else arm type mismatch",
    K0402 => "non-exhaustive match",
    K0403 => "same-module enum wildcard match",
    K0501 => "? used in incompatible return context",
    K0502 => "catch is not exhaustive",
    K0503 => "union error match is not exhaustive",
    K0504 => "cannot destructure opaque Error",
    K0601 => "interface declares more than five methods",
    K0602 => "duplicate method name in interface",
    K0603 => "missing method in impl",
    K0604 => "method signature mismatch in impl",
    K0605 => "type does not implement interface",
    K0606 => "method not found in interface",
    K0607 => "extraneous method in impl",
    K0701 => "spawn outside a scope",
    K0702 => "task result read before join barrier",
    K0703 => "task handle escapes its scope",
    K0801 => "type parameter without interface bound",
    K0802 => "method not in interface bound of type parameter",
    K0803 => "type argument does not satisfy interface bound",
    K0804 => "duplicate type parameter name",
    K0805 => "type parameter name shadows existing type",
    K0806 => "too many type parameters",
    K0807 => "interface used as generic constraint declares more than five methods",
    K0901 => "user-defined generics are not in Core",
    K0902 => "interfaces are not in Core",
    K0903 => "scope/spawn are not in Core",
    K0904 => "arena is not in Core",
    K0905 => "extern/FFI is not in Core",
    K0906 => "attributes are not in Core",
    K0907 => "operator overloading is not in Core",
    K0908 => "async/await are not in Core",
    K1501 => "negative duration",
    K1502 => "invalid deadline type",
    K1503 => "unsupported JSON target",
    K1504 => "invalid HTTP handler",
    K1505 => "invalid HTTP port",
    K1506 => "invalid FromRow function",
    K1507 => "unparseable config target",
}

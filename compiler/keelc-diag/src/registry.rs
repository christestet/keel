//! Append-only registry of stable diagnostic codes used by keelc.

use crate::Code;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RegistryEntry {
    pub code: Code,
    pub summary: &'static str,
}

pub const K0001: Code = Code::new("K0001");
pub const K0002: Code = Code::new("K0002");
pub const K0003: Code = Code::new("K0003");
pub const K0004: Code = Code::new("K0004");
pub const K0101: Code = Code::new("K0101");
pub const K0102: Code = Code::new("K0102");
pub const K0201: Code = Code::new("K0201");
pub const K0202: Code = Code::new("K0202");
pub const K0203: Code = Code::new("K0203");
pub const K0204: Code = Code::new("K0204");
pub const K0301: Code = Code::new("K0301");
pub const K0302: Code = Code::new("K0302");
pub const K0303: Code = Code::new("K0303");
pub const K0401: Code = Code::new("K0401");
pub const K0402: Code = Code::new("K0402");
pub const K0403: Code = Code::new("K0403");
pub const K0501: Code = Code::new("K0501");
pub const K0502: Code = Code::new("K0502");
pub const K0503: Code = Code::new("K0503");
pub const K0601: Code = Code::new("K0601");
pub const K0602: Code = Code::new("K0602");
pub const K0603: Code = Code::new("K0603");
pub const K0604: Code = Code::new("K0604");
pub const K0605: Code = Code::new("K0605");
pub const K0606: Code = Code::new("K0606");
pub const K0607: Code = Code::new("K0607");
pub const K0801: Code = Code::new("K0801");
pub const K0802: Code = Code::new("K0802");
pub const K0803: Code = Code::new("K0803");
pub const K0804: Code = Code::new("K0804");
pub const K0805: Code = Code::new("K0805");
pub const K0806: Code = Code::new("K0806");
pub const K0807: Code = Code::new("K0807");
pub const K0901: Code = Code::new("K0901");
pub const K0902: Code = Code::new("K0902");
pub const K0903: Code = Code::new("K0903");
pub const K0904: Code = Code::new("K0904");
pub const K0905: Code = Code::new("K0905");
pub const K0906: Code = Code::new("K0906");
pub const K0907: Code = Code::new("K0907");
pub const K0908: Code = Code::new("K0908");

pub const ALL_CODES: &[RegistryEntry] = &[
    RegistryEntry {
        code: K0001,
        summary: "unrecognized character",
    },
    RegistryEntry {
        code: K0002,
        summary: "unterminated string literal",
    },
    RegistryEntry {
        code: K0003,
        summary: "syntax error",
    },
    RegistryEntry {
        code: K0004,
        summary: "malformed string interpolation",
    },
    RegistryEntry {
        code: K0101,
        summary: "identifier casing violation",
    },
    RegistryEntry {
        code: K0102,
        summary: "semicolon used as a statement terminator",
    },
    RegistryEntry {
        code: K0201,
        summary: "nullish construct used",
    },
    RegistryEntry {
        code: K0202,
        summary: "implicit numeric conversion",
    },
    RegistryEntry {
        code: K0203,
        summary: "integer overflow rule violation",
    },
    RegistryEntry {
        code: K0204,
        summary: "division or remainder by zero",
    },
    RegistryEntry {
        code: K0301,
        summary: "struct construction missing required field",
    },
    RegistryEntry {
        code: K0302,
        summary: "function signature type annotation required",
    },
    RegistryEntry {
        code: K0303,
        summary: "assignment to immutable binding",
    },
    RegistryEntry {
        code: K0401,
        summary: "if/else arm type mismatch",
    },
    RegistryEntry {
        code: K0402,
        summary: "non-exhaustive match",
    },
    RegistryEntry {
        code: K0403,
        summary: "same-module enum wildcard match",
    },
    RegistryEntry {
        code: K0501,
        summary: "? used in incompatible return context",
    },
    RegistryEntry {
        code: K0502,
        summary: "catch is not exhaustive",
    },
    RegistryEntry {
        code: K0503,
        summary: "union error match is not exhaustive",
    },
    RegistryEntry {
        code: K0601,
        summary: "interface declares more than five methods",
    },
    RegistryEntry {
        code: K0602,
        summary: "duplicate method name in interface",
    },
    RegistryEntry {
        code: K0603,
        summary: "missing method in impl",
    },
    RegistryEntry {
        code: K0604,
        summary: "method signature mismatch in impl",
    },
    RegistryEntry {
        code: K0605,
        summary: "type does not implement interface",
    },
    RegistryEntry {
        code: K0606,
        summary: "method not found in interface",
    },
    RegistryEntry {
        code: K0607,
        summary: "extraneous method in impl",
    },
    RegistryEntry {
        code: K0801,
        summary: "type parameter without interface bound",
    },
    RegistryEntry {
        code: K0802,
        summary: "method not in interface bound of type parameter",
    },
    RegistryEntry {
        code: K0803,
        summary: "type argument does not satisfy interface bound",
    },
    RegistryEntry {
        code: K0804,
        summary: "duplicate type parameter name",
    },
    RegistryEntry {
        code: K0805,
        summary: "type parameter name shadows existing type",
    },
    RegistryEntry {
        code: K0806,
        summary: "too many type parameters",
    },
    RegistryEntry {
        code: K0807,
        summary: "interface used as generic constraint declares more than five methods",
    },
    RegistryEntry {
        code: K0901,
        summary: "user-defined generics are not in Core",
    },
    RegistryEntry {
        code: K0902,
        summary: "interfaces are not in Core",
    },
    RegistryEntry {
        code: K0903,
        summary: "scope/spawn are not in Core",
    },
    RegistryEntry {
        code: K0904,
        summary: "arena is not in Core",
    },
    RegistryEntry {
        code: K0905,
        summary: "extern/FFI is not in Core",
    },
    RegistryEntry {
        code: K0906,
        summary: "attributes are not in Core",
    },
    RegistryEntry {
        code: K0907,
        summary: "operator overloading is not in Core",
    },
    RegistryEntry {
        code: K0908,
        summary: "async/await are not in Core",
    },
];

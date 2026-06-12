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
pub const K0301: Code = Code::new("K0301");
pub const K0302: Code = Code::new("K0302");
pub const K0303: Code = Code::new("K0303");
pub const K0401: Code = Code::new("K0401");
pub const K0402: Code = Code::new("K0402");
pub const K0403: Code = Code::new("K0403");
pub const K0501: Code = Code::new("K0501");
pub const K0502: Code = Code::new("K0502");
pub const K0503: Code = Code::new("K0503");
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

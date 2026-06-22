//! `keel gen`: schema → Keel source (spec ch17, KDR-0104).
//!
//! The first reader is the `proto3` message/enum subset (§17.3). Output is built
//! as an AST and emitted through the shared pretty-printer (`keelc-ast::pretty`),
//! so generated source is `keel fmt`-idempotent by construction and there is no
//! second formatting path (compiler iron rule 3). Every malformed or unsupported
//! input is a `K16xx` diagnostic, never a panic (hard rule 6).
//!
//! Scope is deliberately small (ponytail): messages, enums, scalar/named/repeated
//! fields. Anything else is `K1602` rather than a guessed mapping.

use keelc_ast::pretty::pretty_print;
use keelc_ast::{EnumDecl, FieldDecl, Item, Module, StructDecl, Type, VariantDecl};
use keelc_span::{SourceId, Span, Spanned};

/// A gen diagnostic: stable code + message. Conformance matches on the code.
type GenDiag = (&'static str, String);

const K_MALFORMED: &str = "K1601";
const K_UNSUPPORTED: &str = "K1602";

const DUMMY: Span = Span::empty(SourceId::new(0), 0);

fn spanned(value: impl Into<String>) -> Spanned<String> {
    Spanned::new(value.into(), DUMMY)
}

/// Generate Keel source from a `proto3` schema. Returns the formatted source or
/// the first diagnostic encountered.
pub fn generate(schema: &str) -> Result<String, GenDiag> {
    let items = parse_proto(schema)?;
    Ok(pretty_print(&Module {
        header: None,
        items,
    }))
}

/// proto3 scalar → Keel type name (§17.3). `bytes` is intentionally absent: it
/// has no Core scalar and is reported `K1602` by the caller.
fn scalar_type(name: &str) -> Option<&'static str> {
    Some(match name {
        "double" | "float" => "Float",
        "int32" | "int64" | "uint32" | "uint64" | "sint32" | "sint64" | "fixed32" | "fixed64"
        | "sfixed32" | "sfixed64" => "Int",
        "bool" => "Bool",
        "string" => "String",
        _ => return None,
    })
}

// ---------- tokenizer ----------

/// Split a `.proto` file into tokens. `{ } = ;` are single-char tokens; comments
/// (`//` and `/* */`) and string literals are consumed; everything else is a
/// run of non-space, non-punctuation characters.
fn tokenize(src: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let bytes = src.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else if c == b'/' && i + 1 < bytes.len() && bytes[i + 1] == b'*' {
            i += 2;
            while i + 1 < bytes.len() && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = (i + 2).min(bytes.len());
        } else if c == b'"' {
            // String literal: consumed whole (its contents are never inspected).
            i += 1;
            while i < bytes.len() && bytes[i] != b'"' {
                i += 1;
            }
            i = (i + 1).min(bytes.len());
            tokens.push("\"\"".to_string());
        } else if matches!(c, b'{' | b'}' | b'=' | b';') {
            tokens.push((c as char).to_string());
            i += 1;
        } else {
            let start = i;
            while i < bytes.len()
                && !bytes[i].is_ascii_whitespace()
                && !matches!(bytes[i], b'{' | b'}' | b'=' | b';' | b'"')
            {
                i += 1;
            }
            tokens.push(src[start..i].to_string());
        }
    }
    tokens
}

// ---------- parser ----------

struct Parser<'a> {
    tokens: &'a [String],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&str> {
        self.tokens.get(self.pos).map(String::as_str)
    }

    fn next(&mut self) -> Option<&str> {
        let tok = self.tokens.get(self.pos).map(String::as_str);
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, want: &str) -> Result<(), GenDiag> {
        match self.next() {
            Some(tok) if tok == want => Ok(()),
            other => Err(malformed(format!(
                "expected `{want}`, found {}",
                describe(other)
            ))),
        }
    }

    /// Consume tokens through the next `;` (used for ignored top-level statements).
    fn skip_to_semicolon(&mut self) -> Result<(), GenDiag> {
        while let Some(tok) = self.next() {
            if tok == ";" {
                return Ok(());
            }
        }
        Err(malformed(
            "unterminated statement (missing `;`)".to_string(),
        ))
    }
}

fn parse_proto(src: &str) -> Result<Vec<Item>, GenDiag> {
    let tokens = tokenize(src);
    let mut parser = Parser {
        tokens: &tokens,
        pos: 0,
    };
    let mut items = Vec::new();
    while let Some(tok) = parser.peek() {
        match tok {
            "syntax" | "package" | "import" | "option" => {
                parser.pos += 1;
                parser.skip_to_semicolon()?;
            }
            "message" => {
                parser.pos += 1;
                items.push(parse_message(&mut parser)?);
            }
            "enum" => {
                parser.pos += 1;
                items.push(parse_enum(&mut parser)?);
            }
            ";" => {
                parser.pos += 1;
            }
            other => {
                return Err(malformed(format!("unexpected `{other}` at top level")));
            }
        }
    }
    Ok(items)
}

fn parse_message(parser: &mut Parser) -> Result<Item, GenDiag> {
    let name = ident(parser.next())?;
    parser.expect("{")?;
    let mut fields = Vec::new();
    loop {
        match parser.peek() {
            Some("}") => {
                parser.pos += 1;
                break;
            }
            None => {
                return Err(malformed(
                    "unterminated `message` (missing `}`)".to_string(),
                ))
            }
            _ => fields.push(parse_field(parser)?),
        }
    }
    Ok(Item::Struct(StructDecl {
        name: spanned(name),
        type_params: Vec::new(),
        fields,
        span: DUMMY,
    }))
}

fn parse_field(parser: &mut Parser) -> Result<FieldDecl, GenDiag> {
    let mut repeated = false;
    if parser.peek() == Some("repeated") {
        parser.pos += 1;
        repeated = true;
    }
    let type_tok = ident(parser.next())?;
    // Reject constructs that masquerade as a field's leading token.
    if matches!(
        type_tok.as_str(),
        "oneof" | "map" | "reserved" | "message" | "enum" | "group" | "extensions" | "extend"
    ) || type_tok.starts_with("map")
        || type_tok.contains('<')
    {
        return Err(unsupported(format!(
            "`{type_tok}` is not in the supported subset"
        )));
    }
    let field_name = ident(parser.next())?;
    parser.expect("=")?;
    let number = parser.next();
    if !number.is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit())) {
        return Err(malformed(format!(
            "expected a field number, found {}",
            describe(number)
        )));
    }
    parser.expect(";")?;

    let element = match scalar_type(&type_tok) {
        Some(scalar) => named(scalar),
        None if type_tok == "bytes" => {
            return Err(unsupported("`bytes` has no Core scalar".to_string()))
        }
        None => named(&type_tok), // a message/enum named in the file
    };
    let ty = if repeated { list_of(element) } else { element };
    Ok(FieldDecl {
        name: spanned(field_name),
        ty,
        default: None,
        span: DUMMY,
    })
}

fn parse_enum(parser: &mut Parser) -> Result<Item, GenDiag> {
    let name = ident(parser.next())?;
    parser.expect("{")?;
    let mut variants = Vec::new();
    loop {
        match parser.peek() {
            Some("}") => {
                parser.pos += 1;
                break;
            }
            None => return Err(malformed("unterminated `enum` (missing `}`)".to_string())),
            _ => {
                let variant = ident(parser.next())?;
                parser.expect("=")?;
                let number = parser.next();
                if !number.is_some_and(|n| n.bytes().all(|b| b.is_ascii_digit())) {
                    return Err(malformed(format!(
                        "expected an enum value, found {}",
                        describe(number)
                    )));
                }
                parser.expect(";")?;
                variants.push(VariantDecl {
                    name: spanned(variant),
                    fields: Vec::new(),
                    span: DUMMY,
                });
            }
        }
    }
    Ok(Item::Enum(EnumDecl {
        name: spanned(name),
        type_params: Vec::new(),
        variants,
        span: DUMMY,
    }))
}

// ---------- helpers ----------

fn named(name: &str) -> Type {
    Type::Named {
        name: spanned(name),
        args: Vec::new(),
        span: DUMMY,
    }
}

fn list_of(inner: Type) -> Type {
    Type::Named {
        name: spanned("List"),
        args: vec![inner],
        span: DUMMY,
    }
}

/// A name token must be a plain identifier, not punctuation or EOF.
fn ident(tok: Option<&str>) -> Result<String, GenDiag> {
    match tok {
        Some(t) if !matches!(t, "{" | "}" | "=" | ";") => Ok(t.to_string()),
        other => Err(malformed(format!(
            "expected a name, found {}",
            describe(other)
        ))),
    }
}

fn describe(tok: Option<&str>) -> String {
    match tok {
        Some(t) => format!("`{t}`"),
        None => "end of input".to_string(),
    }
}

fn malformed(detail: String) -> GenDiag {
    (K_MALFORMED, format!("malformed schema: {detail}"))
}

fn unsupported(detail: String) -> GenDiag {
    (
        K_UNSUPPORTED,
        format!("unsupported schema construct: {detail}"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_struct_with_scalars_named_and_repeated() {
        let out = generate(
            "syntax = \"proto3\";\nmessage User { string name = 1; int64 id = 2; }\n\
             message UserList { repeated User users = 1; }",
        )
        .expect("should generate");
        assert!(out.contains("struct User {"));
        assert!(out.contains("name: String"));
        assert!(out.contains("id: Int"));
        assert!(out.contains("users: List<User>"));
    }

    #[test]
    fn malformed_message_is_k1601() {
        let err = generate("message User { string name = 1;").unwrap_err();
        assert_eq!(err.0, "K1601");
    }

    #[test]
    fn bytes_field_is_k1602() {
        let err = generate("message Blob { bytes data = 1; }").unwrap_err();
        assert_eq!(err.0, "K1602");
    }

    #[test]
    fn map_field_is_k1602() {
        let err = generate("message M { map<string, User> m = 1; }").unwrap_err();
        assert_eq!(err.0, "K1602");
    }
}

//! Module-level symbol lookup for definition/hover/completion/documentSymbol.
//!
//! M8 Core has no local-scope resolution map (`keelc-resolve`'s
//! `ResolveOutput` carries diagnostics only, no name/definition index), so
//! this crate does its own light lookup: the lexer's token spans locate the
//! identifier under a cursor, and a flat index of top-level `fn`/`struct`
//! declarations resolves it. This covers module-level go-to-definition and
//! hover/completion for declared and built-in names — it does not resolve
//! locals, parameters, or struct-field access chains.

use keelc_ast::{FunctionDecl, Item, Module, StructDecl, Type};
use keelc_lex::{lex, TokenKind};
use keelc_span::{SourceId, Span};

/// Built-in functions with no declaration site, keyed by name.
pub const BUILTINS: &[(&str, &str)] = &[("print", "fn print(value: String)")];

pub struct TopLevel<'a> {
    pub functions: Vec<&'a FunctionDecl>,
    pub structs: Vec<&'a StructDecl>,
}

#[must_use]
pub fn collect(module: &Module) -> TopLevel<'_> {
    let mut functions = Vec::new();
    let mut structs = Vec::new();
    for item in &module.items {
        match item {
            Item::Function(decl) => functions.push(decl),
            Item::Struct(decl) => structs.push(decl),
            _ => {}
        }
    }
    TopLevel { functions, structs }
}

/// The identifier token, if any, whose span touches `offset` (inclusive on
/// both ends, so a cursor immediately after an identifier still matches it).
#[must_use]
pub fn identifier_at(text: &str, offset: usize) -> Option<(String, Span)> {
    let output = lex(SourceId::new(0), text);
    output.tokens.into_iter().find_map(|token| {
        if token.span.start <= offset && offset <= token.span.end {
            if let TokenKind::Identifier(name) = token.kind {
                return Some((name, token.span));
            }
        }
        None
    })
}

#[must_use]
pub fn find_definition(top: &TopLevel<'_>, name: &str) -> Option<Span> {
    top.functions
        .iter()
        .find(|decl| decl.name.value == name)
        .map(|decl| decl.name.span)
        .or_else(|| {
            top.structs
                .iter()
                .find(|decl| decl.name.value == name)
                .map(|decl| decl.name.span)
        })
}

#[must_use]
pub fn hover_signature(top: &TopLevel<'_>, name: &str) -> Option<String> {
    if let Some(decl) = top.functions.iter().find(|decl| decl.name.value == name) {
        return Some(function_signature(decl));
    }
    BUILTINS
        .iter()
        .find(|(builtin, _)| *builtin == name)
        .map(|(_, signature)| (*signature).to_owned())
}

pub struct CompletionCandidate {
    pub label: String,
    pub detail: String,
    pub is_function: bool,
}

#[must_use]
pub fn completions(top: &TopLevel<'_>, prefix: &str) -> Vec<CompletionCandidate> {
    let mut items = Vec::new();
    for (name, signature) in BUILTINS {
        if name.starts_with(prefix) {
            items.push(CompletionCandidate {
                label: (*name).to_owned(),
                detail: (*signature).to_owned(),
                is_function: true,
            });
        }
    }
    for decl in &top.functions {
        if decl.name.value.starts_with(prefix) {
            items.push(CompletionCandidate {
                label: decl.name.value.clone(),
                detail: function_signature(decl),
                is_function: true,
            });
        }
    }
    for decl in &top.structs {
        if decl.name.value.starts_with(prefix) {
            items.push(CompletionCandidate {
                label: decl.name.value.clone(),
                detail: format!("struct {}", decl.name.value),
                is_function: false,
            });
        }
    }
    items.sort_by(|left, right| left.label.cmp(&right.label));
    items.dedup_by(|left, right| left.label == right.label);
    items
}

#[must_use]
pub fn function_signature(decl: &FunctionDecl) -> String {
    let params: Vec<String> = decl
        .params
        .iter()
        .map(|param| match &param.ty {
            Some(ty) => format!("{}: {}", param.name.value, render_type(ty)),
            None => param.name.value.clone(),
        })
        .collect();
    match &decl.return_type {
        Some(ty) => format!(
            "fn {}({}) -> {}",
            decl.name.value,
            params.join(", "),
            render_type(ty)
        ),
        None => format!("fn {}({})", decl.name.value, params.join(", ")),
    }
}

fn render_type(ty: &Type) -> String {
    match ty {
        Type::Named { name, args, .. } => {
            if args.is_empty() {
                name.value.clone()
            } else {
                let rendered: Vec<String> = args.iter().map(render_type).collect();
                format!("{}<{}>", name.value, rendered.join(", "))
            }
        }
        Type::Union { members, .. } => members
            .iter()
            .map(render_type)
            .collect::<Vec<_>>()
            .join(" | "),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use keelc_parse::parse_with_milestone;

    fn parse(text: &str) -> Module {
        parse_with_milestone(SourceId::new(0), text, 7).module
    }

    #[test]
    fn finds_function_definition_by_call_site_name() {
        let module = parse("fn greet(name: String) -> String {\n    return name\n}\n");
        let top = collect(&module);
        let (name, _) = identifier_at("fn greet(name: String) -> String {", 5).unwrap();
        assert_eq!(name, "greet");
        let span = find_definition(&top, &name).unwrap();
        assert_eq!(span.start, 3);
        assert_eq!(span.end, 8);
    }

    #[test]
    fn renders_function_signature_for_hover() {
        let module = parse("fn greet(name: String) -> String {\n    return name\n}\n");
        let top = collect(&module);
        assert_eq!(
            hover_signature(&top, "greet"),
            Some("fn greet(name: String) -> String".to_owned())
        );
    }

    #[test]
    fn completes_builtin_by_prefix() {
        let module = parse("fn main() {\n}\n");
        let top = collect(&module);
        let items = completions(&top, "pri");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].label, "print");
    }
}

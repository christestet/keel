//! Cross-package symbol linking (spec §6.4, KDR-0044).
//!
//! ponytail: source-level merge. Parse each dependency module the root `use`s,
//! rename its top-level functions and types by the dependency's manifest name,
//! rewrite the root's `module.fn(...)` calls and `module.Type` annotations into
//! the mangled forms — including calls inside string interpolations, whose raw
//! text is re-parsed, mangled, and re-emitted (see `rewrite_interpolations`) —
//! then hand one merged module to the existing single-source pipeline via the
//! pretty-printer. Ceiling — a cross-package call nested inside a *nested*
//! interpolation (`"{f("{g()}")}"`) is not reached; enum *variant* names are not
//! mangled, so cross-package variant-name collisions are unsupported; and the
//! root cannot construct a dependency struct directly (`dep.Point{...}` does not
//! parse). Such cases fail loudly downstream (unknown name/type), never silently
//! mislink. Upgrade path when the corpus needs it: real module namespaces in the
//! resolver/backend so linking stops round-tripping through source (KDR-0044
//! reopening clause).

use crate::manifest;
use keelc_ast::pretty::{pretty_print, pretty_print_expr};
use keelc_ast::{Block, Expr, Item, Module, Pattern, RouteHandler, Stmt, Type};
use keelc_parse::{parse_interpolation_expr, parse_with_milestone};
use keelc_span::{SourceId, Spanned};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

/// Link a root file against its path dependencies, returning the merged source
/// to compile. A single-file program (no adjacent `keel.toml` with
/// dependencies) is returned unchanged, so non-package builds are untouched.
/// Lenient: `manifest::check_workspace` has already rejected any malformed
/// workspace before this runs.
#[must_use]
pub fn link(entry: &Path, root_text: &str, milestone: u32) -> String {
    let deps = manifest::root_dependencies(entry);
    if deps.is_empty() {
        return root_text.to_string();
    }
    let alias_to_dep: BTreeMap<&str, (&Path, &str)> = deps
        .iter()
        .map(|(alias, dir, name)| (alias.as_str(), (dir.as_path(), name.as_str())))
        .collect();

    let mut root = parse_with_milestone(SourceId::new(0), root_text, milestone).module;

    // Deduplicated `std` imports from the root and every merged dependency
    // module (BTreeMap key = path segments → deterministic order, hard rule 7).
    let mut std_uses: BTreeMap<Vec<String>, Item> = BTreeMap::new();
    // Imported dependency module local name → its linked package (name +
    // function/type sets), used to rewrite `local.fn(...)` and `local.Type`.
    let mut modules: BTreeMap<String, Linked> = BTreeMap::new();
    let mut dep_functions: Vec<Item> = Vec::new();

    collect_std_uses(&root, &mut std_uses);

    for item in &root.items {
        let Item::Use(use_decl) = item else { continue };
        let segments: Vec<String> = use_decl.path.iter().map(|s| s.value.clone()).collect();
        let Some(alias) = segments.first() else {
            continue;
        };
        let Some((dep_dir, package)) = alias_to_dep.get(alias.as_str()).copied() else {
            continue; // `std`, self, or (already-diagnosed) unknown alias
        };
        if segments.len() < 2 {
            continue;
        }
        let relative: PathBuf = segments.iter().skip(1).map(String::as_str).collect();
        let module_file = dep_dir.join(relative).with_extension("keel");
        let Ok(module_text) = std::fs::read_to_string(&module_file) else {
            continue; // module file absent → downstream reports the bad call
        };
        let mut dep_module = parse_with_milestone(SourceId::new(0), &module_text, milestone).module;

        collect_std_uses(&dep_module, &mut std_uses);
        let (functions, types) = mangle_dependency(&mut dep_module, package);
        let local_name = segments.last().cloned().unwrap_or_default();
        modules.insert(
            local_name,
            Linked {
                package: package.to_string(),
                functions,
                types,
            },
        );
        for dep_item in dep_module.items {
            if matches!(
                dep_item,
                Item::Function(_) | Item::Struct(_) | Item::Enum(_)
            ) {
                dep_functions.push(dep_item);
            }
        }
    }

    if modules.is_empty() {
        return root_text.to_string();
    }

    walk_module(
        &mut root,
        &mut |expr| {
            rewrite_call_site(expr, &modules);
            rewrite_interpolations(expr, &mut |e| rewrite_call_site(e, &modules), &mut |ty| {
                rewrite_type_ref(ty, &modules)
            });
        },
        &mut |ty| rewrite_type_ref(ty, &modules),
    );

    let mut items: Vec<Item> = Vec::new();
    items.extend(std_uses.into_values());
    items.extend(
        root.items
            .into_iter()
            .filter(|i| !matches!(i, Item::Use(_))),
    );
    items.extend(dep_functions);
    pretty_print(&Module {
        header: root.header,
        items,
    })
}

fn mangle(package: &str, name: &str) -> String {
    format!("{package}__{name}")
}

/// Mangle a dependency **type** name. A type name may not contain `_` and must
/// be UpperCamelCase (K0101), so the `pkg__name` form used for functions is
/// invalid here. Prefix the PascalCased package name instead, keeping the
/// result a valid, deterministic UpperCamelCase identifier.
fn mangle_type(package: &str, name: &str) -> String {
    format!("{}{}", pascal(package), name)
}

/// PascalCase a package name (`my_geo.lib` → `MyGeoLib`) for use as a type-name
/// prefix. Splits on the separators a manifest name may contain.
fn pascal(s: &str) -> String {
    s.split(['_', '.', '-'])
        .filter(|seg| !seg.is_empty())
        .map(|seg| {
            let mut chars = seg.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(chars).collect::<String>()
            })
        })
        .collect()
}

/// Collect the module's `use std.*` declarations into `into`, keyed by path so
/// duplicates across merged modules collapse to one deterministic entry.
fn collect_std_uses(module: &Module, into: &mut BTreeMap<Vec<String>, Item>) {
    for item in &module.items {
        if let Item::Use(use_decl) = item {
            let segments: Vec<String> = use_decl.path.iter().map(|s| s.value.clone()).collect();
            if segments.first().map(String::as_str) == Some("std") {
                into.insert(segments, item.clone());
            }
        }
    }
}

/// Rename every top-level function and type in a dependency module (declarations
/// and internal references) by the package prefix, and return the original
/// function and type names so the root can map `local.fn`/`local.Type` to the
/// `package__name` forms.
fn mangle_dependency(module: &mut Module, package: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let mut functions = BTreeSet::new();
    let mut types = BTreeSet::new();
    for item in &module.items {
        match item {
            Item::Function(decl) => {
                functions.insert(decl.name.value.clone());
            }
            Item::Struct(decl) => {
                types.insert(decl.name.value.clone());
            }
            Item::Enum(decl) => {
                types.insert(decl.name.value.clone());
            }
            _ => {}
        }
    }
    for item in &mut module.items {
        match item {
            Item::Function(decl) => decl.name.value = mangle(package, &decl.name.value),
            Item::Struct(decl) => decl.name.value = mangle_type(package, &decl.name.value),
            Item::Enum(decl) => decl.name.value = mangle_type(package, &decl.name.value),
            _ => {}
        }
    }
    walk_module(
        module,
        &mut |expr| {
            rewrite_dep_symbol(expr, package, &functions, &types);
            rewrite_interpolations(
                expr,
                &mut |e| rewrite_dep_symbol(e, package, &functions, &types),
                &mut |ty| rewrite_dep_type(ty, package, &types),
            );
        },
        &mut |ty| rewrite_dep_type(ty, package, &types),
    );
    (functions, types)
}

/// Mangle a reference to one of the dependency's own top-level symbols: a call
/// to a sibling function, or the construction of one of its structs.
fn rewrite_dep_symbol(
    expr: &mut Expr,
    package: &str,
    functions: &BTreeSet<String>,
    types: &BTreeSet<String>,
) {
    match expr {
        Expr::Call { callee, .. } => {
            if let Expr::Name(name) = callee.as_mut() {
                if functions.contains(name.value.as_str()) {
                    name.value = mangle(package, &name.value);
                }
            }
        }
        Expr::StructLiteral { name, .. } if types.contains(name.value.as_str()) => {
            name.value = mangle_type(package, &name.value);
        }
        _ => {}
    }
}

/// Mangle a reference to one of the dependency's own types in a type position.
fn rewrite_dep_type(ty: &mut Type, package: &str, types: &BTreeSet<String>) {
    if let Type::Named { name, .. } = ty {
        if types.contains(name.value.as_str()) {
            name.value = mangle_type(package, &name.value);
        }
    }
}

/// Rewrite the symbols inside a string literal's interpolations. Interpolation
/// bodies are stored as raw text (the AST keeps them unparsed), so the module
/// walk cannot reach them: re-parse each body, apply the same `rewrite`/
/// `rewrite_ty` mangling used for real expressions, then re-emit the body and
/// splice it back into both the interpolation entry and the literal's text.
///
/// ponytail: single level only. A cross-package call nested inside a *nested*
/// interpolation (`"{f("{g()}")}"`) is not rewritten — `rewrite` is the plain
/// symbol mangler, not this function, so it does not recurse back in. Bind the
/// inner result in a `let` if you hit that.
fn rewrite_interpolations(
    expr: &mut Expr,
    rewrite: &mut impl FnMut(&mut Expr),
    rewrite_ty: &mut impl FnMut(&mut Type),
) {
    let Expr::String(lit) = expr else { return };
    for i in 0..lit.value.interpolations.len() {
        let original = lit.value.interpolations[i].value.clone();
        let source = lit.value.interpolations[i].span.source;
        let Some(mut sub) = parse_interpolation_expr(source, &original) else {
            continue; // malformed body → downstream K0004 on the original text
        };
        walk_expr(&mut sub, rewrite, rewrite_ty);
        let new_text = pretty_print_expr(&sub);
        if new_text != original {
            let old_needle = format!("{{{original}}}");
            let new_needle = format!("{{{new_text}}}");
            lit.value.text = lit.value.text.replacen(&old_needle, &new_needle, 1);
            lit.value.interpolations[i].value = new_text;
        }
    }
}

/// An imported dependency module, keyed in `modules` by its root-local name.
/// `functions`/`types` hold the *original* (unmangled) names so root-side
/// references can be matched before rewriting to the `package__name` form.
struct Linked {
    package: String,
    functions: BTreeSet<String>,
    types: BTreeSet<String>,
}

/// Rewrite `local.fn(args)` — a `MethodCall` whose receiver names an imported
/// dependency module — into the free call `package__fn(args)`.
fn rewrite_call_site(expr: &mut Expr, modules: &BTreeMap<String, Linked>) {
    let replacement = match expr {
        Expr::MethodCall {
            receiver,
            method,
            args,
            span,
        } => match receiver.as_ref() {
            Expr::Name(local) => match modules.get(local.value.as_str()) {
                Some(dep) if dep.functions.contains(method.value.as_str()) => Some(Expr::Call {
                    callee: Box::new(Expr::Name(Spanned::new(
                        mangle(&dep.package, &method.value),
                        method.span,
                    ))),
                    type_args: Vec::new(),
                    args: std::mem::take(args),
                    span: *span,
                }),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    };
    if let Some(new) = replacement {
        *expr = new;
    }
}

/// Rewrite a `local.Type` type annotation in the root into the merged
/// `package__Type`. The parser folds a dotted type name into one string
/// (`"math.Point"`), so split on the first `.` to recover module + type.
fn rewrite_type_ref(ty: &mut Type, modules: &BTreeMap<String, Linked>) {
    let Type::Named { name, .. } = ty else { return };
    let Some((local, type_name)) = name.value.split_once('.') else {
        return;
    };
    if let Some(dep) = modules.get(local) {
        if dep.types.contains(type_name) {
            name.value = mangle_type(&dep.package, type_name);
        }
    }
}

// --- exhaustive expression + type walk (mutating) ------------------------
// Visits every expression (`f`) and every type node (`t`) in a module's
// declarations and bodies, applying the callback before recursing so a node a
// callback replaces still has its new children walked. `t` reaches struct/enum
// field types, function signatures, `let` annotations, generic type arguments,
// and pattern type annotations — every position a dependency type can appear.

fn walk_module(module: &mut Module, f: &mut impl FnMut(&mut Expr), t: &mut impl FnMut(&mut Type)) {
    for item in &mut module.items {
        match item {
            Item::Struct(decl) => {
                for field in &mut decl.fields {
                    walk_type(&mut field.ty, t);
                }
            }
            Item::Enum(decl) => {
                for variant in &mut decl.variants {
                    for field in &mut variant.fields {
                        walk_type(&mut field.ty, t);
                    }
                }
            }
            Item::Function(decl) => {
                for param in &mut decl.params {
                    if let Some(ty) = &mut param.ty {
                        walk_type(ty, t);
                    }
                    if let Some(default) = &mut param.default {
                        walk_expr(default, f, t);
                    }
                }
                if let Some(ret) = &mut decl.return_type {
                    walk_type(ret, t);
                }
                if let Some(body) = &mut decl.body {
                    walk_block(body, f, t);
                }
            }
            Item::Test(test) => walk_block(&mut test.body, f, t),
            _ => {}
        }
    }
}

/// Visit a type node and every type nested inside it (union members, generic
/// arguments), applying `t` to each.
fn walk_type(ty: &mut Type, t: &mut impl FnMut(&mut Type)) {
    t(ty);
    match ty {
        Type::Named { args, .. } => {
            for arg in args {
                walk_type(arg, t);
            }
        }
        Type::Union { members, .. } => {
            for member in members {
                walk_type(member, t);
            }
        }
    }
}

/// Visit the type annotation on a pattern (`Err(err: sql.Error)`) and recurse
/// into its sub-patterns.
fn walk_pattern_types(pattern: &mut Pattern, t: &mut impl FnMut(&mut Type)) {
    if let Pattern::Name { args, ty, .. } = pattern {
        if let Some(ty) = ty {
            walk_type(ty, t);
        }
        for arg in args {
            walk_pattern_types(arg, t);
        }
    }
}

fn walk_block(block: &mut Block, f: &mut impl FnMut(&mut Expr), t: &mut impl FnMut(&mut Type)) {
    for stmt in &mut block.statements {
        match stmt {
            Stmt::Let { ty, value, .. } => {
                if let Some(ty) = ty {
                    walk_type(ty, t);
                }
                walk_expr(value, f, t);
            }
            Stmt::Assert { value, .. } => walk_expr(value, f, t),
            Stmt::Assign { target, value, .. } => {
                walk_expr(target, f, t);
                walk_expr(value, f, t);
            }
            Stmt::Return {
                value: Some(value), ..
            } => walk_expr(value, f, t),
            Stmt::Expr(expr) => walk_expr(expr, f, t),
            Stmt::Return { value: None, .. } | Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }
}

fn walk_expr(expr: &mut Expr, f: &mut impl FnMut(&mut Expr), t: &mut impl FnMut(&mut Type)) {
    f(expr);
    match expr {
        Expr::Unary { expr, .. }
        | Expr::Spawn { expr, .. }
        | Expr::Question { expr, .. }
        | Expr::Field { target: expr, .. } => walk_expr(expr, f, t),
        Expr::Binary { left, right, .. } => {
            walk_expr(left, f, t);
            walk_expr(right, f, t);
        }
        Expr::Call {
            callee,
            type_args,
            args,
            ..
        } => {
            walk_expr(callee, f, t);
            for ty in type_args {
                walk_type(ty, t);
            }
            for arg in args {
                walk_expr(&mut arg.value, f, t);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            walk_expr(receiver, f, t);
            for arg in args {
                walk_expr(&mut arg.value, f, t);
            }
        }
        Expr::StructLiteral {
            type_args, fields, ..
        } => {
            for ty in type_args {
                walk_type(ty, t);
            }
            for field in fields {
                walk_expr(&mut field.value, f, t);
            }
        }
        Expr::If {
            condition,
            then_block,
            else_branch,
            ..
        } => {
            walk_expr(condition, f, t);
            walk_block(then_block, f, t);
            if let Some(branch) = else_branch {
                walk_expr(branch, f, t);
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            walk_expr(scrutinee, f, t);
            for arm in arms {
                walk_pattern_types(&mut arm.pattern, t);
                if let Some(guard) = &mut arm.guard {
                    walk_expr(guard, f, t);
                }
                walk_expr(&mut arm.value, f, t);
            }
        }
        Expr::While {
            condition, body, ..
        } => {
            walk_expr(condition, f, t);
            walk_block(body, f, t);
        }
        Expr::Scope { deadline, body, .. } => {
            if let Some(deadline) = deadline {
                walk_expr(deadline, f, t);
            }
            walk_block(body, f, t);
        }
        Expr::Arena { body, .. } => walk_block(body, f, t),
        Expr::Block(block) => walk_block(block, f, t),
        Expr::Catch { expr, arms, .. } => {
            walk_expr(expr, f, t);
            for arm in arms {
                walk_pattern_types(&mut arm.pattern, t);
                if let Some(guard) = &mut arm.guard {
                    walk_expr(guard, f, t);
                }
                walk_expr(&mut arm.value, f, t);
            }
        }
        Expr::Return {
            value: Some(value), ..
        } => walk_expr(value, f, t),
        Expr::Router { routes, .. } => {
            for route in routes {
                match &mut route.handler {
                    RouteHandler::Expr(expr) => walk_expr(expr, f, t),
                    RouteHandler::Closure { body, .. } => walk_expr(body, f, t),
                }
            }
        }
        Expr::Return { value: None, .. }
        | Expr::Missing(_)
        | Expr::Int(_)
        | Expr::Float(_)
        | Expr::String(_)
        | Expr::Char(_)
        | Expr::Bool(_)
        | Expr::Name(_)
        | Expr::Wildcard(_)
        | Expr::Unit(_) => {}
    }
}

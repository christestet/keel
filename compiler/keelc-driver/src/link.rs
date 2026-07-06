//! Cross-package symbol linking (spec §6.4, KDR-0044).
//!
//! ponytail: source-level merge. Parse each dependency module the root `use`s,
//! rename its top-level functions by the dependency's manifest name, rewrite the
//! root's `module.fn(...)` calls into the mangled free function, then hand one
//! merged module to the existing single-source pipeline via the pretty-printer.
//! Ceiling — dependency **functions** only, and a cross-package call must sit
//! outside string interpolation (the AST keeps interpolations as raw text, so a
//! call written `"{dep.f()}"` is not rewritten). Non-function dependency items
//! and interpolated calls fail loudly downstream (unknown name/type), never
//! silently mislink. Upgrade path when the corpus needs it: real module
//! namespaces in the resolver/backend so linking stops round-tripping through
//! source (KDR-0044 reopening clause).

use crate::manifest;
use keelc_ast::pretty::pretty_print;
use keelc_ast::{Block, Expr, Item, Module, RouteHandler, Stmt};
use keelc_parse::parse_with_milestone;
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
    // Imported dependency module local name → (package name, its function set),
    // used to rewrite `local.fn(...)` at the call site.
    let mut modules: BTreeMap<String, (String, BTreeSet<String>)> = BTreeMap::new();
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
        let functions = mangle_dependency(&mut dep_module, package);
        let local_name = segments.last().cloned().unwrap_or_default();
        modules.insert(local_name, (package.to_string(), functions));
        for dep_item in dep_module.items {
            if matches!(dep_item, Item::Function(_)) {
                dep_functions.push(dep_item);
            }
        }
    }

    if modules.is_empty() {
        return root_text.to_string();
    }

    walk_module_exprs(&mut root, &mut |expr| rewrite_call_site(expr, &modules));

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

/// Rename every top-level function in a dependency module (declaration and
/// internal call sites) by the package prefix, and return the original function
/// names so the root can map `local.fn` to `package__fn`.
fn mangle_dependency(module: &mut Module, package: &str) -> BTreeSet<String> {
    let functions: BTreeSet<String> = module
        .items
        .iter()
        .filter_map(|item| match item {
            Item::Function(decl) => Some(decl.name.value.clone()),
            _ => None,
        })
        .collect();
    for item in &mut module.items {
        if let Item::Function(decl) = item {
            decl.name.value = mangle(package, &decl.name.value);
        }
    }
    walk_module_exprs(module, &mut |expr| {
        if let Expr::Call { callee, .. } = expr {
            if let Expr::Name(name) = callee.as_mut() {
                if functions.contains(name.value.as_str()) {
                    name.value = mangle(package, &name.value);
                }
            }
        }
    });
    functions
}

/// Rewrite `local.fn(args)` — a `MethodCall` whose receiver names an imported
/// dependency module — into the free call `package__fn(args)`.
fn rewrite_call_site(expr: &mut Expr, modules: &BTreeMap<String, (String, BTreeSet<String>)>) {
    let replacement = match expr {
        Expr::MethodCall {
            receiver,
            method,
            args,
            span,
        } => match receiver.as_ref() {
            Expr::Name(local) => match modules.get(local.value.as_str()) {
                Some((package, functions)) if functions.contains(method.value.as_str()) => {
                    Some(Expr::Call {
                        callee: Box::new(Expr::Name(Spanned::new(
                            mangle(package, &method.value),
                            method.span,
                        ))),
                        type_args: Vec::new(),
                        args: std::mem::take(args),
                        span: *span,
                    })
                }
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

// --- exhaustive expression walk (mutating) -------------------------------
// Visits every expression in a module's function and test bodies, applying `f`
// before recursing so a node `f` replaces still has its new children walked.

fn walk_module_exprs(module: &mut Module, f: &mut impl FnMut(&mut Expr)) {
    for item in &mut module.items {
        match item {
            Item::Function(decl) => {
                for param in &mut decl.params {
                    if let Some(default) = &mut param.default {
                        walk_expr(default, f);
                    }
                }
                if let Some(body) = &mut decl.body {
                    walk_block(body, f);
                }
            }
            Item::Test(test) => walk_block(&mut test.body, f),
            _ => {}
        }
    }
}

fn walk_block(block: &mut Block, f: &mut impl FnMut(&mut Expr)) {
    for stmt in &mut block.statements {
        match stmt {
            Stmt::Let { value, .. } | Stmt::Assert { value, .. } => walk_expr(value, f),
            Stmt::Assign { target, value, .. } => {
                walk_expr(target, f);
                walk_expr(value, f);
            }
            Stmt::Return {
                value: Some(value), ..
            } => walk_expr(value, f),
            Stmt::Expr(expr) => walk_expr(expr, f),
            Stmt::Return { value: None, .. } | Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }
}

fn walk_expr(expr: &mut Expr, f: &mut impl FnMut(&mut Expr)) {
    f(expr);
    match expr {
        Expr::Unary { expr, .. }
        | Expr::Spawn { expr, .. }
        | Expr::Question { expr, .. }
        | Expr::Field { target: expr, .. } => walk_expr(expr, f),
        Expr::Binary { left, right, .. } => {
            walk_expr(left, f);
            walk_expr(right, f);
        }
        Expr::Call { callee, args, .. } => {
            walk_expr(callee, f);
            for arg in args {
                walk_expr(&mut arg.value, f);
            }
        }
        Expr::MethodCall { receiver, args, .. } => {
            walk_expr(receiver, f);
            for arg in args {
                walk_expr(&mut arg.value, f);
            }
        }
        Expr::StructLiteral { fields, .. } => {
            for field in fields {
                walk_expr(&mut field.value, f);
            }
        }
        Expr::If {
            condition,
            then_block,
            else_branch,
            ..
        } => {
            walk_expr(condition, f);
            walk_block(then_block, f);
            if let Some(branch) = else_branch {
                walk_expr(branch, f);
            }
        }
        Expr::Match {
            scrutinee, arms, ..
        } => {
            walk_expr(scrutinee, f);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    walk_expr(guard, f);
                }
                walk_expr(&mut arm.value, f);
            }
        }
        Expr::While {
            condition, body, ..
        } => {
            walk_expr(condition, f);
            walk_block(body, f);
        }
        Expr::Scope { deadline, body, .. } => {
            if let Some(deadline) = deadline {
                walk_expr(deadline, f);
            }
            walk_block(body, f);
        }
        Expr::Arena { body, .. } => walk_block(body, f),
        Expr::Block(block) => walk_block(block, f),
        Expr::Catch { expr, arms, .. } => {
            walk_expr(expr, f);
            for arm in arms {
                if let Some(guard) = &mut arm.guard {
                    walk_expr(guard, f);
                }
                walk_expr(&mut arm.value, f);
            }
        }
        Expr::Return {
            value: Some(value), ..
        } => walk_expr(value, f),
        Expr::Router { routes, .. } => {
            for route in routes {
                match &mut route.handler {
                    RouteHandler::Expr(expr) => walk_expr(expr, f),
                    RouteHandler::Closure { body, .. } => walk_expr(body, f),
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

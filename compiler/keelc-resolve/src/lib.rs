//! Name resolution and early semantic diagnostics for Keel Core.

use keelc_ast::{Block, Expr, Item, Module, Stmt};
use keelc_diag::{registry, Diagnostic};
use keelc_span::Spanned;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolveOutput {
    pub diagnostics: Vec<Diagnostic>,
}

#[must_use]
pub fn resolve(module: &Module) -> ResolveOutput {
    Resolver::new(module).resolve()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum BindingKind {
    Immutable,
    Mutable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Binding {
    name: String,
    kind: BindingKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StructInfo {
    name: String,
    fields: Vec<StructFieldInfo>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StructFieldInfo {
    name: String,
    has_default: bool,
}

struct Resolver<'a> {
    module: &'a Module,
    structs: Vec<StructInfo>,
    scopes: Vec<Vec<Binding>>,
    diagnostics: Vec<Diagnostic>,
}

impl<'a> Resolver<'a> {
    fn new(module: &'a Module) -> Self {
        Self {
            module,
            structs: collect_structs(module),
            scopes: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn resolve(mut self) -> ResolveOutput {
        for item in &self.module.items {
            match item {
                Item::Function(function) => {
                    if let Some(body) = &function.body {
                        self.push_scope();
                        for param in &function.params {
                            self.define(&param.name, BindingKind::Immutable);
                        }
                        self.resolve_block(body);
                        self.pop_scope();
                    }
                }
                Item::Test(test) => {
                    self.push_scope();
                    self.resolve_block(&test.body);
                    self.pop_scope();
                }
                Item::Struct(_) | Item::Enum(_) | Item::Use(_) => {}
            }
        }

        ResolveOutput {
            diagnostics: self.diagnostics,
        }
    }

    fn resolve_block(&mut self, block: &Block) {
        self.push_scope();
        for statement in &block.statements {
            self.resolve_stmt(statement);
        }
        self.pop_scope();
    }

    fn resolve_stmt(&mut self, statement: &Stmt) {
        match statement {
            Stmt::Let {
                mutable,
                name,
                value,
                ..
            } => {
                self.resolve_expr(value);
                let kind = if *mutable {
                    BindingKind::Mutable
                } else {
                    BindingKind::Immutable
                };
                self.define(name, kind);
            }
            Stmt::Assign { target, value, .. } => {
                self.check_assignment_target(target);
                self.resolve_expr(target);
                self.resolve_expr(value);
            }
            Stmt::Return { value, .. } => {
                if let Some(value) = value {
                    self.resolve_expr(value);
                }
            }
            Stmt::Assert { value, .. } | Stmt::Expr(value) => self.resolve_expr(value),
            Stmt::Break(_) | Stmt::Continue(_) => {}
        }
    }

    fn resolve_expr(&mut self, expr: &Expr) {
        match expr {
            Expr::Missing(_)
            | Expr::Int(_)
            | Expr::Float(_)
            | Expr::String(_)
            | Expr::Char(_)
            | Expr::Bool(_)
            | Expr::Name(_)
            | Expr::Wildcard(_) => {}
            Expr::Unary { expr, .. } | Expr::Question { expr, .. } => self.resolve_expr(expr),
            Expr::Binary { left, right, .. } => {
                self.resolve_expr(left);
                self.resolve_expr(right);
            }
            Expr::Call { callee, args, .. } => {
                self.resolve_expr(callee);
                for arg in args {
                    self.resolve_expr(arg);
                }
            }
            Expr::Field { target, .. } => self.resolve_expr(target),
            Expr::StructLiteral { name, fields, .. } => {
                self.check_struct_literal(name, fields);
                for field in fields {
                    self.resolve_expr(&field.value);
                }
            }
            Expr::If {
                condition,
                then_block,
                else_branch,
                ..
            } => {
                self.resolve_expr(condition);
                self.resolve_block(then_block);
                if let Some(else_branch) = else_branch {
                    self.resolve_expr(else_branch);
                }
            }
            Expr::Match {
                scrutinee, arms, ..
            } => {
                self.resolve_expr(scrutinee);
                for arm in arms {
                    self.push_scope();
                    self.resolve_expr(&arm.value);
                    self.pop_scope();
                }
            }
            Expr::While {
                condition, body, ..
            } => {
                self.resolve_expr(condition);
                self.resolve_block(body);
            }
            Expr::Block(block) => self.resolve_block(block),
            Expr::Catch {
                expr,
                error_name,
                arms,
                ..
            } => {
                self.resolve_expr(expr);
                self.push_scope();
                self.define(error_name, BindingKind::Immutable);
                for arm in arms {
                    self.push_scope();
                    self.resolve_expr(&arm.value);
                    self.pop_scope();
                }
                self.pop_scope();
            }
            Expr::Return { value, .. } => {
                if let Some(value) = value {
                    self.resolve_expr(value);
                }
            }
        }
    }

    fn check_assignment_target(&mut self, target: &Expr) {
        if let Expr::Name(name) = target {
            if self.binding_kind(&name.value) == Some(BindingKind::Immutable) {
                self.diagnostics.push(Diagnostic::error(
                    registry::K0303,
                    name.span,
                    format!("cannot assign to immutable binding `{}`", name.value),
                ));
            }
        }
    }

    fn check_struct_literal(
        &mut self,
        name: &Spanned<String>,
        fields: &[keelc_ast::StructLiteralField],
    ) {
        let Some(info) = self.structs.iter().find(|info| info.name == name.value) else {
            return;
        };

        let missing = info.fields.iter().find(|field| {
            !field.has_default
                && !fields
                    .iter()
                    .any(|provided| provided.name.value == field.name)
        });

        if let Some(field) = missing {
            self.diagnostics.push(Diagnostic::error(
                registry::K0301,
                name.span,
                format!(
                    "struct `{}` is missing required field `{}`",
                    name.value, field.name
                ),
            ));
        }
    }

    fn define(&mut self, name: &Spanned<String>, kind: BindingKind) {
        if let Some(scope) = self.scopes.last_mut() {
            scope.push(Binding {
                name: name.value.clone(),
                kind,
            });
        }
    }

    fn binding_kind(&self, name: &str) -> Option<BindingKind> {
        self.scopes
            .iter()
            .rev()
            .flat_map(|scope| scope.iter().rev())
            .find(|binding| binding.name == name)
            .map(|binding| binding.kind)
    }

    fn push_scope(&mut self) {
        self.scopes.push(Vec::new());
    }

    fn pop_scope(&mut self) {
        let _ = self.scopes.pop();
    }
}

fn collect_structs(module: &Module) -> Vec<StructInfo> {
    let mut structs = Vec::new();
    for item in &module.items {
        if let Item::Struct(decl) = item {
            let fields = decl
                .fields
                .iter()
                .map(|field| StructFieldInfo {
                    name: field.name.value.clone(),
                    has_default: field.default.is_some(),
                })
                .collect();
            structs.push(StructInfo {
                name: decl.name.value.clone(),
                fields,
            });
        }
    }
    structs.sort_by(|left, right| left.name.cmp(&right.name));
    structs
}

#[cfg(test)]
mod tests {
    use super::resolve;
    use keelc_diag::registry;
    use keelc_parse::parse;
    use keelc_span::SourceId;

    #[test]
    fn reports_assignment_to_immutable_let() {
        let output = parse(SourceId::new(0), "fn main() {\nlet x = 1\nx = 2\n}\n");
        assert!(output.diagnostics.is_empty());

        let resolved = resolve(&output.module);

        assert_eq!(resolved.diagnostics[0].code, registry::K0303);
    }

    #[test]
    fn allows_assignment_to_mut_binding() {
        let output = parse(SourceId::new(0), "fn main() {\nmut x = 1\nx = 2\n}\n");
        assert!(output.diagnostics.is_empty());

        let resolved = resolve(&output.module);

        assert!(resolved.diagnostics.is_empty());
    }

    #[test]
    fn reports_missing_required_struct_field() {
        let output = parse(
            SourceId::new(0),
            "struct User {\nid: Int\nname: String\n}\nfn main() {\nlet u = User{ id: 1 }\n}\n",
        );
        assert!(output.diagnostics.is_empty());

        let resolved = resolve(&output.module);

        assert_eq!(resolved.diagnostics[0].code, registry::K0301);
    }

    #[test]
    fn permits_missing_struct_field_with_default() {
        let output = parse(
            SourceId::new(0),
            "struct Config {\nhost: String\nport: Int = 8080\n}\nfn main() {\nlet c = Config{ host: \"localhost\" }\n}\n",
        );
        assert!(output.diagnostics.is_empty());

        let resolved = resolve(&output.module);

        assert!(resolved.diagnostics.is_empty());
    }
}

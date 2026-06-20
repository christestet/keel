//! AST definitions and pretty-printer foundation for keelc.

pub mod pretty;

use keelc_span::{Span, Spanned};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Module {
    pub header: Option<Spanned<String>>,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Item {
    Use(UseDecl),
    Struct(StructDecl),
    Enum(EnumDecl),
    Function(FunctionDecl),
    Interface(InterfaceDecl),
    Impl(ImplDecl),
    Test(TestDecl),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TypeParam {
    pub name: Spanned<String>,
    pub bound: Option<Spanned<String>>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UseDecl {
    pub path: Vec<Spanned<String>>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructDecl {
    pub name: Spanned<String>,
    pub type_params: Vec<TypeParam>,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FieldDecl {
    pub name: Spanned<String>,
    pub ty: Type,
    pub default: Option<Expr>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumDecl {
    pub name: Spanned<String>,
    pub type_params: Vec<TypeParam>,
    pub variants: Vec<VariantDecl>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VariantDecl {
    pub name: Spanned<String>,
    pub fields: Vec<FieldDecl>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionDecl {
    pub name: Spanned<String>,
    pub type_params: Vec<TypeParam>,
    pub params: Vec<Param>,
    pub return_type: Option<Type>,
    pub body: Option<Block>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceDecl {
    pub name: Spanned<String>,
    pub methods: Vec<FunctionDecl>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImplDecl {
    pub interface_name: Spanned<String>,
    pub type_name: Spanned<String>,
    pub type_args: Vec<Type>,
    pub methods: Vec<FunctionDecl>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Param {
    pub name: Spanned<String>,
    pub ty: Option<Type>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestDecl {
    pub name: Spanned<String>,
    pub body: Block,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    Named {
        name: Spanned<String>,
        args: Vec<Type>,
        span: Span,
    },
    Union {
        members: Vec<Type>,
        span: Span,
    },
}

impl Type {
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Type::Named { span, .. } | Type::Union { span, .. } => *span,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Stmt {
    Let {
        mutable: bool,
        name: Spanned<String>,
        ty: Option<Type>,
        value: Expr,
        span: Span,
    },
    Assign {
        target: Expr,
        value: Expr,
        span: Span,
    },
    Return {
        value: Option<Expr>,
        span: Span,
    },
    Break(Span),
    Continue(Span),
    Assert {
        value: Expr,
        span: Span,
    },
    Expr(Expr),
}

impl Stmt {
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Stmt::Let { span, .. }
            | Stmt::Assign { span, .. }
            | Stmt::Return { span, .. }
            | Stmt::Assert { span, .. } => *span,
            Stmt::Break(span) | Stmt::Continue(span) => *span,
            Stmt::Expr(expr) => expr.span(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StringLiteral {
    pub text: String,
    pub interpolations: Vec<Spanned<String>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr {
    Missing(Span),
    Int(Spanned<String>),
    Float(Spanned<String>),
    String(Spanned<StringLiteral>),
    Char(Spanned<String>),
    Bool(Spanned<bool>),
    Name(Spanned<String>),
    Wildcard(Span),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        span: Span,
    },
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        type_args: Vec<Type>,
        args: Vec<Expr>,
        span: Span,
    },
    Field {
        target: Box<Expr>,
        field: Spanned<String>,
        span: Span,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: Spanned<String>,
        args: Vec<Expr>,
        span: Span,
    },
    StructLiteral {
        name: Spanned<String>,
        type_args: Vec<Type>,
        fields: Vec<StructLiteralField>,
        span: Span,
    },
    If {
        condition: Box<Expr>,
        then_block: Block,
        else_branch: Option<Box<Expr>>,
        span: Span,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },
    While {
        condition: Box<Expr>,
        body: Block,
        span: Span,
    },
    Scope {
        deadline: Option<Box<Expr>>,
        body: Block,
        span: Span,
    },
    Spawn {
        expr: Box<Expr>,
        span: Span,
    },
    Block(Block),
    Question {
        expr: Box<Expr>,
        span: Span,
    },
    Catch {
        expr: Box<Expr>,
        error_name: Spanned<String>,
        arms: Vec<MatchArm>,
        span: Span,
    },
    Return {
        value: Option<Box<Expr>>,
        span: Span,
    },
    /// `http.Router{ "METHOD /path": handler, ... }` — the compiler-known route
    /// table (KDR-0031). The only construct with string keys and the only place a
    /// handler closure may appear; both are contained here, not general features.
    Router {
        routes: Vec<Route>,
        span: Span,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Route {
    pub pattern: Spanned<String>,
    pub handler: RouteHandler,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouteHandler {
    /// A handler value expected to be a function name resolving to
    /// `fn(http.Request) -> http.Response`. Held as a general expression so the
    /// resolver can reject non-name / wrong-signature values with `K1504`.
    Expr(Box<Expr>),
    /// `fn(req) => expr` — a single-expression handler that may capture the
    /// enclosing scope. Restricted to route values; not a general closure.
    Closure {
        param: Spanned<String>,
        body: Box<Expr>,
        span: Span,
    },
}

impl RouteHandler {
    #[must_use]
    pub fn span(&self) -> Span {
        match self {
            RouteHandler::Expr(expr) => expr.span(),
            RouteHandler::Closure { span, .. } => *span,
        }
    }
}

impl Expr {
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Expr::Missing(span)
            | Expr::Wildcard(span)
            | Expr::Unary { span, .. }
            | Expr::Binary { span, .. }
            | Expr::Call { span, .. }
            | Expr::Field { span, .. }
            | Expr::MethodCall { span, .. }
            | Expr::StructLiteral { span, .. }
            | Expr::If { span, .. }
            | Expr::Match { span, .. }
            | Expr::While { span, .. }
            | Expr::Scope { span, .. }
            | Expr::Spawn { span, .. }
            | Expr::Question { span, .. }
            | Expr::Catch { span, .. }
            | Expr::Return { span, .. }
            | Expr::Router { span, .. } => *span,
            Expr::Int(value) | Expr::Float(value) | Expr::Char(value) | Expr::Name(value) => {
                value.span
            }
            Expr::String(value) => value.span,
            Expr::Bool(value) => value.span,
            Expr::Block(block) => block.span,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UnaryOp {
    Negate,
    Not,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    And,
    Or,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructLiteralField {
    pub name: Spanned<String>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub value: Expr,
    pub span: Span,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pattern {
    Name {
        name: Spanned<String>,
        args: Vec<Pattern>,
        span: Span,
    },
    Wildcard(Span),
}

impl Pattern {
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Pattern::Name { span, .. } | Pattern::Wildcard(span) => *span,
        }
    }
}

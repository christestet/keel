//! Keel Intermediate Representation (KIR).
//!
//! KIR is a small, explicitly-typed, desugared IR consumed by all backends.
//! The Go backend no longer sees the AST; lowering translates the typed AST
//! (plus [`TypeContext`] information from `keelc-types`) into KIR, desugaring
//! control-flow features such as `?` and `catch` into explicit match/return
//! sequences.

pub mod lower;

use keelc_types::TypeInfo;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Module {
    pub name: Option<String>,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Item {
    Struct(StructDecl),
    Enum(EnumDecl),
    Function(FunctionDecl),
    Interface(InterfaceDecl),
    Impl(ImplDecl),
    Test(TestDecl),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<Variant>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Field {
    pub name: String,
    pub ty: TypeInfo,
    pub default: Option<Expr>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: TypeInfo,
    pub body: Block,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Param {
    pub name: String,
    pub ty: TypeInfo,
    pub default: Option<Expr>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InterfaceDecl {
    pub name: String,
    pub methods: Vec<Method>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Method {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: TypeInfo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImplDecl {
    pub interface_name: String,
    pub type_name: String,
    pub methods: Vec<FunctionDecl>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestDecl {
    pub name: String,
    pub body: Block,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub ty: TypeInfo,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Stmt {
    Let {
        name: String,
        ty: TypeInfo,
        value: Expr,
    },
    Var {
        name: String,
        ty: TypeInfo,
    },
    Assign {
        target: Expr,
        value: Expr,
    },
    Return {
        value: Option<Expr>,
    },
    Break,
    Continue,
    Assert {
        value: Expr,
        line: usize,
    },
    Expr(Expr),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Expr {
    Int(String),
    Float(String),
    String(StringLiteral),
    Char(char),
    Bool(bool),
    Unit,
    Name(String),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
        ty: TypeInfo,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        ty: TypeInfo,
    },
    Call {
        callee: Box<Expr>,
        type_args: Vec<TypeInfo>,
        args: Vec<Expr>,
        ty: TypeInfo,
    },
    Field {
        target: Box<Expr>,
        field: String,
        ty: TypeInfo,
    },
    MethodCall {
        receiver: Box<Expr>,
        method: String,
        args: Vec<Expr>,
        arg_types: Vec<TypeInfo>,
        ty: TypeInfo,
    },
    StructLiteral {
        name: String,
        fields: Vec<(String, Expr)>,
        ty: TypeInfo,
    },
    If {
        condition: Box<Expr>,
        then_block: Block,
        else_block: Block,
        ty: TypeInfo,
    },
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
        ty: TypeInfo,
    },
    While {
        condition: Box<Expr>,
        body: Block,
    },
    Scope {
        deadline: Option<Box<Expr>>,
        body: Block,
        ty: TypeInfo,
        error_ty: Option<TypeInfo>,
    },
    Spawn {
        expr: Box<Expr>,
        ty: TypeInfo,
    },
    Payload {
        value: Box<Expr>,
        index: usize,
        ty: TypeInfo,
    },
    Block(Block),
    Return {
        value: Option<Box<Expr>>,
    },
    /// `http.Router{ ... }` route table (KDR-0031). Handlers are lowered to a
    /// pattern string plus either a named function or an inline closure.
    Router {
        routes: Vec<Route>,
        ty: TypeInfo,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Route {
    pub pattern: String,
    pub handler: RouteHandler,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RouteHandler {
    /// A function name resolving to `fn(http.Request) -> http.Response`.
    Named(String),
    /// `fn(param) => body`, capturing the enclosing scope.
    Closure { param: String, body: Box<Expr> },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StringLiteral {
    pub parts: Vec<StringPart>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StringPart {
    Text(String),
    Expr { expr: Box<Expr>, ty: TypeInfo },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub value: Expr,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pattern {
    Name {
        /// A variant tag (`Ok`, `NoRows`, an enum variant) when `is_binding` is
        /// false, or the bound variable name when `is_binding` is true.
        name: String,
        args: Vec<Pattern>,
        payload_types: Vec<TypeInfo>,
        /// `true`: bind `name` to the matched value (a sub-payload or, at the
        /// arm head, the whole scrutinee). `false`: match when `tag == name`.
        is_binding: bool,
        /// Typed binding `x: T` (KDR-0038): match only when the runtime tag is
        /// one of these. `None` matches unconditionally.
        type_test: Option<Vec<String>>,
    },
    /// `()` — the unit payload; matches always, binds nothing.
    Unit,
    Wildcard,
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

impl From<keelc_ast::UnaryOp> for UnaryOp {
    fn from(op: keelc_ast::UnaryOp) -> Self {
        match op {
            keelc_ast::UnaryOp::Negate => Self::Negate,
            keelc_ast::UnaryOp::Not => Self::Not,
        }
    }
}

impl From<keelc_ast::BinaryOp> for BinaryOp {
    fn from(op: keelc_ast::BinaryOp) -> Self {
        match op {
            keelc_ast::BinaryOp::Add => Self::Add,
            keelc_ast::BinaryOp::Subtract => Self::Subtract,
            keelc_ast::BinaryOp::Multiply => Self::Multiply,
            keelc_ast::BinaryOp::Divide => Self::Divide,
            keelc_ast::BinaryOp::Remainder => Self::Remainder,
            keelc_ast::BinaryOp::Equal => Self::Equal,
            keelc_ast::BinaryOp::NotEqual => Self::NotEqual,
            keelc_ast::BinaryOp::Less => Self::Less,
            keelc_ast::BinaryOp::LessEqual => Self::LessEqual,
            keelc_ast::BinaryOp::Greater => Self::Greater,
            keelc_ast::BinaryOp::GreaterEqual => Self::GreaterEqual,
            keelc_ast::BinaryOp::And => Self::And,
            keelc_ast::BinaryOp::Or => Self::Or,
        }
    }
}

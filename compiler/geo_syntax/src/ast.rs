use crate::token::Span;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Program {
    pub imports: Vec<Import>,
    pub type_aliases: Vec<TypeAlias>,
    pub consts: Vec<ConstDecl>,
    pub structs: Vec<StructDecl>,
    pub enums: Vec<EnumDecl>,
    pub externs: Vec<ExternFunction>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    pub path: Vec<String>,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeAlias {
    pub name: String,
    pub ty: Type,
    pub is_public: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConstDecl {
    pub name: String,
    pub ty: Type,
    pub value: Expr,
    pub is_public: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternFunction {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub is_public: bool,
    pub source_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDecl {
    pub name: String,
    pub fields: Vec<Field>,
    pub is_public: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumDecl {
    pub name: String,
    pub variants: Vec<EnumVariant>,
    pub is_public: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnumVariant {
    pub name: String,
    pub value: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub return_type: Type,
    pub is_public: bool,
    pub body: Vec<Stmt>,
    pub span: Span,
    pub statement_spans: Vec<Span>,
    pub expression_spans: Vec<Span>,
    pub statement_expression_ranges: Vec<(usize, usize)>,
    pub source_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    Unit,
    Int,
    Bool,
    Char,
    String,
    Usize,
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    Array(Box<Type>),
    Slice(Box<Type>),
    Reference { mutable: bool, inner: Box<Type> },
    Pointer(Box<Type>),
    Named(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    Return(Option<Expr>),
    Let {
        name: String,
        ty: Option<Type>,
        mutable: bool,
        value: Expr,
    },
    Assign {
        name: String,
        op: Option<BinaryOp>,
        value: Expr,
    },
    PointerAssign {
        pointer: Expr,
        op: Option<BinaryOp>,
        value: Expr,
    },
    PlaceAssign {
        target: Expr,
        op: Option<BinaryOp>,
        value: Expr,
    },
    If {
        condition: Expr,
        then_body: Vec<Stmt>,
        else_body: Vec<Stmt>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    For {
        name: String,
        start: Expr,
        end: Expr,
        inclusive: bool,
        body: Vec<Stmt>,
    },
    Loop(Vec<Stmt>),
    Unsafe(Vec<Stmt>),
    Break,
    Continue,
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Expr {
    Int(i64),
    TypedInt {
        value: i64,
        ty: Type,
    },
    Bool(bool),
    Char(char),
    String(String),
    Null,
    Var(String),
    Unary {
        op: UnaryOp,
        expr: Box<Expr>,
    },
    Cast {
        expr: Box<Expr>,
        ty: Type,
    },
    SizeOf(Type),
    AlignOf(Type),
    OffsetOf {
        ty: Type,
        field: String,
    },
    Match {
        value: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    If {
        condition: Box<Expr>,
        then_value: Box<Expr>,
        else_value: Box<Expr>,
    },
    Block {
        statements: Vec<Stmt>,
        value: Box<Expr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        name: String,
        args: Vec<Expr>,
    },
    Struct {
        name: String,
        fields: Vec<(String, Expr)>,
    },
    Array(Vec<Expr>),
    Field {
        base: Box<Expr>,
        name: String,
    },
    Index {
        base: Box<Expr>,
        index: Box<Expr>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub value: Expr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchPattern {
    Wildcard,
    Int(i64),
    Bool(bool),
    EnumVariant { enum_name: String, variant: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
    BitNot,
    AddressOf,
    MutableAddressOf,
    Deref,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Or,
    And,
    BitOr,
    BitXor,
    BitAnd,
    ShiftLeft,
    ShiftRight,
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

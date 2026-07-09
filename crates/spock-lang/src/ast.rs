//! Abstract syntax tree (docs/spec/v0.md §3). Spanned, unresolved.

use crate::span::Span;

#[derive(Clone, Debug)]
pub struct Ident {
    pub name: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct File {
    pub tables: Vec<TableDecl>,
    pub seeds: Vec<SeedBlock>,
}

#[derive(Clone, Debug)]
pub struct TableDecl {
    pub name: Ident,
    pub items: Vec<TableItem>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum TableItem {
    Field(FieldDecl),
    /// `key (a, b, ...)`
    Key {
        fields: Vec<Ident>,
        span: Span,
    },
    /// `unique (a, b, ...)`
    Unique {
        fields: Vec<Ident>,
        span: Span,
    },
}

#[derive(Clone, Debug)]
pub struct FieldDecl {
    pub is_key: bool,
    pub name: Ident,
    pub ty: TypeExpr,
    pub optional: bool,
    pub unique: bool,
    pub default: Option<DefaultExpr>,
    pub on_delete: Option<OnDeleteClause>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct TypeExpr {
    pub kind: TypeExprKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum TypeExprKind {
    Text,
    Int,
    Bool,
    Timestamp,
    Uuid,
    /// A reference: the named table's key.
    Named(String),
}

#[derive(Clone, Debug)]
pub enum DefaultExpr {
    Auto(Span),
    Now(Span),
    Lit(Lit),
}

impl DefaultExpr {
    pub fn span(&self) -> Span {
        match self {
            DefaultExpr::Auto(s) | DefaultExpr::Now(s) => *s,
            DefaultExpr::Lit(lit) => lit.span(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum Lit {
    Str(String, Span),
    Int(i64, Span),
    Bool(bool, Span),
}

impl Lit {
    pub fn span(&self) -> Span {
        match self {
            Lit::Str(_, s) | Lit::Int(_, s) | Lit::Bool(_, s) => *s,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnDeleteKind {
    Restrict,
    Cascade,
}

#[derive(Clone, Debug)]
pub struct OnDeleteClause {
    pub kind: OnDeleteKind,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct SeedBlock {
    pub stmts: Vec<SeedStmt>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct SeedStmt {
    pub binding: Option<Ident>,
    pub table: Ident,
    pub fields: Vec<(Ident, SeedValue)>,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum SeedValue {
    Lit(Lit),
    /// Reference to an earlier seed binding.
    Binding(Ident),
}

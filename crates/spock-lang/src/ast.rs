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
    pub records: Vec<RecordDecl>,
    pub fns: Vec<FnDecl>,
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
    Float,
    Bool,
    Timestamp,
    Uuid,
    /// A reference: the named table's key.
    Named(String),
    /// A closed-set type: `"a" | "b" | ...` (RFD 0013). Members carry
    /// spans for per-member diagnostics (E043). The checker enforces ≥2
    /// distinct non-empty members and rejects the type outside a stored
    /// table field (E034/E036/E043).
    Set(Vec<SetMember>),
}

#[derive(Clone, Debug)]
pub struct SetMember {
    pub value: String,
    pub span: Span,
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
    Float(f64, Span),
    Bool(bool, Span),
}

impl Lit {
    pub fn span(&self) -> Span {
        match self {
            Lit::Str(_, s) | Lit::Int(_, s) | Lit::Float(_, s) | Lit::Bool(_, s) => *s,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OnDeleteKind {
    Restrict,
    Cascade,
    /// `set null` — legal only on optional references (E040).
    SetNull,
}

#[derive(Clone, Debug)]
pub struct OnDeleteClause {
    pub kind: OnDeleteKind,
    pub span: Span,
}

/// `record name { field: type ... }` — a named wire shape (§3). Items
/// reuse the table grammar so the checker can reject table-only syntax
/// (keys, uniques, defaults, on delete) with precise spans.
#[derive(Clone, Debug)]
pub struct RecordDecl {
    pub name: Ident,
    pub items: Vec<TableItem>,
    pub span: Span,
}

/// `[mut] fn name(params) -> ret ! codes { sql("...") ... }` (§3).
#[derive(Clone, Debug)]
pub struct FnDecl {
    pub name: Ident,
    /// `mut fn` — the fn may write. Unmarked fns are reads, and the
    /// engine enforces it per statement at load (§7.4, RFD 0012).
    pub mutates: bool,
    pub params: Vec<ParamDecl>,
    pub ret: RetDecl,
    /// Declared error codes (the `! a | b` clause), possibly empty.
    pub errors: Vec<Ident>,
    /// The escape body: one or more `unchecked sql(...)` statements in
    /// execution order; the last produces the return value (§7.4).
    pub body: Vec<SqlEscape>,
    pub span: Span,
}

/// One `unchecked sql("...")` escape: exactly one SQL statement, verbatim.
#[derive(Clone, Debug)]
pub struct SqlEscape {
    pub sql: String,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub struct ParamDecl {
    pub name: Ident,
    pub ty: TypeExpr,
    pub optional: bool,
    pub span: Span,
}

/// A fn return: `t`, `t?`, or `[t]` where `t` is a table or record name,
/// or a builtin scalar type (`int`, `text`, …).
#[derive(Clone, Debug)]
pub struct RetDecl {
    pub arity: RetArity,
    pub target: RetTarget,
    pub span: Span,
}

#[derive(Clone, Debug)]
pub enum RetTarget {
    /// A table or record name, resolved by the checker (E037).
    Named(Ident),
    /// A builtin scalar; the kind is never `Named` here.
    Scalar(TypeExprKind, Span),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetArity {
    /// `-> t`: exactly one row.
    One,
    /// `-> t?`: zero or one row.
    Maybe,
    /// `-> [t]`: any number of rows.
    Many,
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

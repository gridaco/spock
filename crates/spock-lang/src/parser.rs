//! Recursive-descent parser (docs/spec/v0.md §3). Fail-fast: the first
//! syntax error aborts the parse; the checker (many-error) runs after.

use crate::ast::*;
use crate::diag::Diagnostic;
use crate::lexer::{Token, TokenKind, CONTEXTUAL_KEYWORDS, SOFT_KEYWORDS};
use crate::span::Span;

pub fn parse(tokens: Vec<Token>) -> Result<File, Diagnostic> {
    let (tokens, leading) = split_docs(tokens);
    Parser {
        tokens,
        leading,
        pos: 0,
    }
    .file()
}

/// One doc-comment line lifted out of the token stream (RFD 0016).
struct DocLine {
    inner: bool,
    text: String,
    span: Span,
}

/// Split the raw token stream into the *real* tokens — fed to the recursive
/// descent unchanged — and a parallel `leading` array where `leading[i]` holds
/// the doc lines immediately preceding real token `i`. Keeping docs out of the
/// descent leaves the grammar's two-token lookahead untouched (RFD 0016).
fn split_docs(tokens: Vec<Token>) -> (Vec<Token>, Vec<Vec<DocLine>>) {
    let mut real = Vec::new();
    let mut leading: Vec<Vec<DocLine>> = Vec::new();
    let mut pending: Vec<DocLine> = Vec::new();
    for tok in tokens {
        if let TokenKind::Doc { inner, text } = tok.kind {
            pending.push(DocLine {
                inner,
                text,
                span: tok.span,
            });
        } else {
            leading.push(std::mem::take(&mut pending));
            real.push(tok);
        }
    }
    // The lexer always ends with a (non-Doc) Eof, so `pending` is flushed and
    // `real`/`leading` have equal, ≥1 length.
    (real, leading)
}

/// Join doc lines with `\n`, trimming trailing blank lines; an all-empty run
/// yields `None` (a lone `///` is a harmless no-op, RFD 0016 §3).
fn join_doc<'a>(lines: impl Iterator<Item = &'a DocLine>) -> Option<String> {
    let joined = lines
        .map(|l| l.text.as_str())
        .collect::<Vec<_>>()
        .join("\n");
    let trimmed = joined.trim_end();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

struct Parser {
    tokens: Vec<Token>,
    /// Parallel to `tokens`: the doc lines preceding each real token (RFD
    /// 0016). Attachment points drain their group; whatever survives the
    /// parse documented nothing and is flagged by [`Parser::dangling_doc`].
    leading: Vec<Vec<DocLine>>,
    pos: usize,
}

impl Parser {
    /// Join and remove the *outer* (`///`) doc lines preceding real token
    /// `at` — the doc for the item that starts there. Removing them marks the
    /// group consumed, so a `///` with no owner survives for the final scan.
    fn take_outer_doc(&mut self, at: usize) -> Option<String> {
        let group = &mut self.leading[at];
        let joined = join_doc(group.iter().filter(|l| !l.inner));
        group.retain(|l| l.inner);
        joined
    }

    /// Join and remove the *inner* (`//!`) doc lines preceding real token
    /// `at`. Legal only at `at == 0` (the file preamble → the contract);
    /// inner lines surviving anywhere else are flagged `L012`.
    fn take_inner_doc(&mut self, at: usize) -> Option<String> {
        let group = &mut self.leading[at];
        let joined = join_doc(group.iter().filter(|l| l.inner));
        group.retain(|l| !l.inner);
        joined
    }

    /// Attach the outer docs preceding a table/record item to it, but only if
    /// it is a field: docs on a `key`/`unique`/`check` row item are left
    /// unconsumed, so the final scan flags them (`L011`).
    fn attach_field_doc(&mut self, item: TableItem, at: usize) -> TableItem {
        if let TableItem::Field(mut f) = item {
            f.doc = self.take_outer_doc(at);
            TableItem::Field(f)
        } else {
            item
        }
    }

    /// Every doc line left unconsumed after the parse documented nothing: an
    /// outer run out of position (`L011`), or a `//!` past the preamble
    /// (`L012`). Returns the earliest offender in source order; empty content
    /// is a no-op and trips neither (RFD 0016 §9).
    fn dangling_doc(&self) -> Option<Diagnostic> {
        for line in self.leading.iter().flatten() {
            if line.text.is_empty() {
                continue;
            }
            return Some(if line.inner {
                Diagnostic::new(
                    "L012",
                    "`//!` documents the whole file and must come before the first declaration; use `///` to document this item",
                    line.span,
                )
            } else {
                Diagnostic::new(
                    "L011",
                    "this doc comment documents nothing — put `///` directly before the item it documents, or use `//` for an ordinary comment",
                    line.span,
                )
            });
        }
        None
    }

    fn peek(&self) -> &Token {
        &self.tokens[self.pos.min(self.tokens.len() - 1)]
    }

    fn peek2(&self) -> &Token {
        &self.tokens[(self.pos + 1).min(self.tokens.len() - 1)]
    }

    fn bump(&mut self) -> Token {
        let tok = self.tokens[self.pos.min(self.tokens.len() - 1)].clone();
        if self.pos < self.tokens.len() - 1 {
            self.pos += 1;
        }
        tok
    }

    fn expect(&mut self, kind: TokenKind, what: &str) -> Result<Token, Diagnostic> {
        if self.peek().kind == kind {
            Ok(self.bump())
        } else {
            Err(self.unexpected(what))
        }
    }

    fn unexpected(&self, expected: &str) -> Diagnostic {
        let tok = self.peek();
        Diagnostic::new(
            "L010",
            format!("expected {expected}, found {}", tok.kind.describe()),
            tok.span,
        )
    }

    fn ident(&mut self, what: &str) -> Result<Ident, Diagnostic> {
        match &self.peek().kind {
            TokenKind::Ident(_) => {
                let tok = self.bump();
                let TokenKind::Ident(name) = tok.kind else {
                    unreachable!()
                };
                Ok(Ident {
                    name,
                    span: tok.span,
                })
            }
            _ => Err(self.unexpected(what)),
        }
    }

    // file = { table_decl | record_decl | error_decl | fn_decl | seed_block }
    fn file(&mut self) -> Result<File, Diagnostic> {
        // `//!` lines in the preamble (before any declaration) document the
        // contract itself (RFD 0016).
        let doc = self.take_inner_doc(0);
        let mut tables = Vec::new();
        let mut records = Vec::new();
        let mut errors = Vec::new();
        let mut fns = Vec::new();
        let mut seeds = Vec::new();
        loop {
            // `///` lines here document whichever declaration follows. `at` is
            // captured before the `auth`/`mut` prefixes so the doc attaches
            // across them (they precede the modifier keyword).
            let at = self.pos;
            match self.peek().kind {
                TokenKind::KwTable => {
                    let mut decl = self.table_decl()?;
                    decl.doc = self.take_outer_doc(at);
                    tables.push(decl);
                }
                TokenKind::KwAuth => {
                    // `auth table ...` — the identity anchor (RFD 0014).
                    // Mirrors the `mut fn` modifier dispatch below.
                    let start = self.bump().span;
                    if self.peek().kind != TokenKind::KwTable {
                        return Err(self.unexpected("`table` (only a table can be `auth`)"));
                    }
                    let mut decl = self.table_decl()?;
                    decl.auth = Some(start);
                    decl.span = start.to(decl.span);
                    decl.doc = self.take_outer_doc(at);
                    tables.push(decl);
                }
                TokenKind::KwRecord => {
                    let mut decl = self.record_decl()?;
                    decl.doc = self.take_outer_doc(at);
                    records.push(decl);
                }
                TokenKind::KwError => {
                    let mut decl = self.error_decl()?;
                    decl.doc = self.take_outer_doc(at);
                    errors.push(decl);
                }
                TokenKind::KwFn => {
                    let mut decl = self.fn_decl(false)?;
                    decl.doc = self.take_outer_doc(at);
                    fns.push(decl);
                }
                TokenKind::KwMut => {
                    let start = self.bump().span;
                    if self.peek().kind != TokenKind::KwFn {
                        return Err(self.unexpected("`fn` (only a fn can be `mut`)"));
                    }
                    let mut decl = self.fn_decl(true)?;
                    decl.span = start.to(decl.span);
                    decl.doc = self.take_outer_doc(at);
                    fns.push(decl);
                }
                // a `seed` block is not a documentable entity — a `///` before
                // it is left unconsumed and flagged as dangling below.
                TokenKind::KwSeed => seeds.push(self.seed_block()?),
                TokenKind::Eof => break,
                _ => {
                    return Err(self.unexpected(
                        "`table`, `auth table`, `record`, `error`, `fn`, `mut fn`, or `seed`",
                    ));
                }
            }
        }
        // Any doc line that attached to nothing documents nothing (RFD 0016 §9).
        if let Some(d) = self.dangling_doc() {
            return Err(d);
        }
        Ok(File {
            doc,
            tables,
            records,
            errors,
            fns,
            seeds,
        })
    }

    // error_decl = "error" ident
    fn error_decl(&mut self) -> Result<ErrorDecl, Diagnostic> {
        let start = self.expect(TokenKind::KwError, "`error`")?.span;
        let name = self.ident("error code")?;
        Ok(ErrorDecl {
            doc: None,
            span: start.to(name.span),
            name,
        })
    }

    // table_decl = "table" ident "{" { table_item } "}"
    fn table_decl(&mut self) -> Result<TableDecl, Diagnostic> {
        let start = self.expect(TokenKind::KwTable, "`table`")?.span;
        let name = self.ident("table name")?;
        self.expect(TokenKind::LBrace, "`{`")?;
        let mut items = Vec::new();
        let end = loop {
            match self.peek().kind {
                TokenKind::RBrace => break self.bump().span,
                TokenKind::Eof => return Err(self.unexpected("`}`")),
                _ => {
                    let at = self.pos;
                    let item = self.table_item()?;
                    items.push(self.attach_field_doc(item, at));
                }
            }
        };
        Ok(TableDecl {
            doc: None,
            name,
            items,
            auth: None,
            span: start.to(end),
        })
    }

    // record_decl = "record" ident "{" { table_item } "}"
    // (items reuse the table grammar; the checker rejects table-only
    // constructs with precise spans — E033)
    fn record_decl(&mut self) -> Result<RecordDecl, Diagnostic> {
        let start = self.expect(TokenKind::KwRecord, "`record`")?.span;
        let name = self.ident("record name")?;
        self.expect(TokenKind::LBrace, "`{`")?;
        let mut items = Vec::new();
        let end = loop {
            match self.peek().kind {
                TokenKind::RBrace => break self.bump().span,
                TokenKind::Eof => return Err(self.unexpected("`}`")),
                _ => {
                    let at = self.pos;
                    let item = self.table_item()?;
                    items.push(self.attach_field_doc(item, at));
                }
            }
        };
        Ok(RecordDecl {
            doc: None,
            name,
            items,
            span: start.to(end),
        })
    }

    // fn_decl = ["mut"] "fn" ident "(" [param {"," param} [","]] ")" "->" ret
    //           ["!" ident {"|" ident}]
    //           "{" escape { escape } "}"
    // escape  = "unchecked" "sql" "(" string ")"
    // (the caller has already consumed a leading `mut`, if any)
    fn fn_decl(&mut self, mutates: bool) -> Result<FnDecl, Diagnostic> {
        let start = self.expect(TokenKind::KwFn, "`fn`")?.span;
        let name = self.ident("fn name")?;
        self.expect(TokenKind::LParen, "`(`")?;
        let mut params = Vec::new();
        while self.peek().kind != TokenKind::RParen {
            // `///` before a parameter (on its own line) documents it.
            let at = self.pos;
            let pname = self.ident("parameter name")?;
            let pstart = pname.span;
            self.expect(TokenKind::Colon, "`:`")?;
            let ty = self.type_expr()?;
            let mut pend = ty.span;
            let optional = if self.peek().kind == TokenKind::Question {
                pend = self.bump().span;
                true
            } else {
                false
            };
            params.push(ParamDecl {
                doc: self.take_outer_doc(at),
                name: pname,
                ty,
                optional,
                span: pstart.to(pend),
            });
            match self.peek().kind {
                TokenKind::Comma => {
                    self.bump();
                }
                TokenKind::RParen => {}
                _ => return Err(self.unexpected("`,` or `)`")),
            }
        }
        self.expect(TokenKind::RParen, "`)`")?;
        self.expect(TokenKind::Arrow, "`->`")?;
        let ret = self.ret_decl()?;

        let mut errors = Vec::new();
        if self.peek().kind == TokenKind::Bang {
            self.bump();
            errors.push(self.ident("an error code")?);
            while self.peek().kind == TokenKind::Pipe {
                self.bump();
                errors.push(self.ident("an error code")?);
            }
        }

        self.expect(TokenKind::LBrace, "`{`")?;
        // one or more escapes, in execution order; the last one answers
        let mut body = vec![self.sql_escape()?];
        while self.peek().kind != TokenKind::RBrace {
            body.push(self.sql_escape()?);
        }
        let end = self.expect(TokenKind::RBrace, "`}`")?.span;

        Ok(FnDecl {
            doc: None,
            name,
            mutates,
            params,
            ret,
            errors,
            body,
            span: start.to(end),
        })
    }

    // escape = "unchecked" "sql" "(" string ")" — `unchecked` and `sql`
    // are contextual: keywords only here, identifiers everywhere else.
    // The marker is forced — the checker cannot verify the SQL, and the
    // language marks its own limits (RFD 0011).
    fn sql_escape(&mut self) -> Result<SqlEscape, Diagnostic> {
        let marker = self.ident("`unchecked`")?;
        if marker.name == CONTEXTUAL_KEYWORDS[1] {
            return Err(Diagnostic::new(
                "L010",
                "a fn body must acknowledge the escape: write `unchecked sql(\"...\")` — the checker cannot verify SQL (RFD 0011)",
                marker.span,
            ));
        }
        if marker.name != CONTEXTUAL_KEYWORDS[0] {
            return Err(Diagnostic::new(
                "L010",
                format!(
                    "expected `unchecked`, found identifier `{}` (a fn body is one or more `unchecked sql(\"...\")` statements)",
                    marker.name
                ),
                marker.span,
            ));
        }
        let escape = self.ident("`sql`")?;
        if escape.name != CONTEXTUAL_KEYWORDS[1] {
            return Err(Diagnostic::new(
                "L010",
                format!(
                    "expected `sql`, found identifier `{}` (a fn body is one or more `unchecked sql(\"...\")` statements)",
                    escape.name
                ),
                escape.span,
            ));
        }
        self.expect(TokenKind::LParen, "`(`")?;
        let tok = self.peek().clone();
        let sql = match tok.kind {
            TokenKind::Str(s) => {
                self.bump();
                s
            }
            _ => return Err(self.unexpected("a string literal (the SQL statement)")),
        };
        self.expect(TokenKind::RParen, "`)`")?;
        Ok(SqlEscape {
            sql,
            span: marker.span.to(tok.span),
        })
    }

    // ret = "[" ret_target "]" | ret_target ["?"] — a table or record
    // name, or a builtin scalar type
    fn ret_decl(&mut self) -> Result<RetDecl, Diagnostic> {
        if self.peek().kind == TokenKind::LBracket {
            let start = self.bump().span;
            let target = self.ret_target()?;
            let end = self.expect(TokenKind::RBracket, "`]`")?.span;
            return Ok(RetDecl {
                arity: RetArity::Many,
                target,
                span: start.to(end),
            });
        }
        let target = self.ret_target()?;
        let start = match &target {
            RetTarget::Named(ident) => ident.span,
            RetTarget::Scalar(_, span) => *span,
        };
        if self.peek().kind == TokenKind::Question {
            let end = self.bump().span;
            return Ok(RetDecl {
                arity: RetArity::Maybe,
                target,
                span: start.to(end),
            });
        }
        Ok(RetDecl {
            arity: RetArity::One,
            span: start,
            target,
        })
    }

    fn ret_target(&mut self) -> Result<RetTarget, Diagnostic> {
        let tok = self.peek().clone();
        let scalar = match &tok.kind {
            TokenKind::KwText => Some(TypeExprKind::Text),
            TokenKind::KwInt => Some(TypeExprKind::Int),
            TokenKind::KwFloat => Some(TypeExprKind::Float),
            TokenKind::KwBool => Some(TypeExprKind::Bool),
            TokenKind::KwTimestamp => Some(TypeExprKind::Timestamp),
            TokenKind::KwUuid => Some(TypeExprKind::Uuid),
            _ => None,
        };
        if let Some(kind) = scalar {
            self.bump();
            return Ok(RetTarget::Scalar(kind, tok.span));
        }
        Ok(RetTarget::Named(
            self.ident("a type, table, or record name")?,
        ))
    }

    fn table_item(&mut self) -> Result<TableItem, Diagnostic> {
        match (&self.peek().kind, &self.peek2().kind) {
            // `key (` → composite key; `key ident` → inline-key field
            (TokenKind::KwKey, TokenKind::LParen) => {
                let start = self.bump().span;
                let (fields, end) = self.ident_group()?;
                Ok(TableItem::Key {
                    fields,
                    span: start.to(end),
                })
            }
            // `unique (` at item position → unique group
            (TokenKind::KwUnique, TokenKind::LParen) => {
                let start = self.bump().span;
                let (fields, end) = self.ident_group()?;
                Ok(TableItem::Unique {
                    fields,
                    span: start.to(end),
                })
            }
            // `check (` at item position → a row check (RFD 0013)
            (TokenKind::KwCheck, TokenKind::LParen) => self.check_decl(),
            // any other `check` at item position is a misplaced/misordered
            // check — name both legal forms and the order law (L-D)
            (TokenKind::KwCheck, _) => Err(Diagnostic::new(
                "L010",
                "`check` here starts a row check and must name its fields: `check (a, b) fn_name`; a field check is written after a field's type, before `unique`: `name: type check fn_name`",
                self.peek().span,
            )),
            _ => Ok(TableItem::Field(self.field_decl()?)),
        }
    }

    // check_decl = "check" "(" ident "," ident { "," ident } ")" ident
    // — a row check (RFD 0013), mirroring unique_decl but naming a fn. Its
    // own group parse (not ident_group) so a single-field `check(v)` — the
    // parenthesized field check — gets a message that names the fix (L-D).
    fn check_decl(&mut self) -> Result<TableItem, Diagnostic> {
        let start = self.expect(TokenKind::KwCheck, "`check`")?.span;
        self.expect(TokenKind::LParen, "`(`")?;
        let mut fields = vec![self.ident("a field name")?];
        if self.peek().kind == TokenKind::RParen {
            return Err(Diagnostic::new(
                "L010",
                "a field check takes a bare fn name (`check valid_x`), no parentheses; a parenthesized `check (a, b) fn` is a row check naming two or more fields",
                self.peek().span,
            ));
        }
        self.expect(
            TokenKind::Comma,
            "`,` (a row check names two or more fields)",
        )?;
        fields.push(self.ident("a field name")?);
        while self.peek().kind == TokenKind::Comma {
            self.bump();
            fields.push(self.ident("a field name")?);
        }
        let group_end = self.expect(TokenKind::RParen, "`)`")?.span;
        // The validator name is a bare trailing ident with no delimiter, so
        // omitting it would swallow the next field's name — guard the common
        // case (a field line follows) with a message at the group (L-D).
        if matches!(self.peek().kind, TokenKind::Ident(_)) && self.peek2().kind == TokenKind::Colon
        {
            return Err(Diagnostic::new(
                "L010",
                "row check is missing its validator fn name: write `check (a, b) fn_name`",
                group_end,
            ));
        }
        let fn_name = self.ident("a validator fn name")?;
        Ok(TableItem::Check {
            fields,
            span: start.to(fn_name.span),
            fn_name,
        })
    }

    // "(" ident "," ident { "," ident } ")"  — two or more (§3)
    fn ident_group(&mut self) -> Result<(Vec<Ident>, Span), Diagnostic> {
        self.expect(TokenKind::LParen, "`(`")?;
        let mut fields = vec![self.ident("field name")?];
        self.expect(TokenKind::Comma, "`,` (groups name two or more fields)")?;
        fields.push(self.ident("field name")?);
        while self.peek().kind == TokenKind::Comma {
            self.bump();
            fields.push(self.ident("field name")?);
        }
        let end = self.expect(TokenKind::RParen, "`)`")?.span;
        Ok((fields, end))
    }

    // field_decl = ["key"] ident ":" type ["?"] ["unique"] ["=" default]
    //              ["on" "delete" ("restrict"|"cascade"|"set" "null")]
    fn field_decl(&mut self) -> Result<FieldDecl, Diagnostic> {
        let is_key = if self.peek().kind == TokenKind::KwKey {
            self.bump();
            true
        } else {
            false
        };
        let name = self.ident("field name")?;
        let start = name.span;
        self.expect(TokenKind::Colon, "`:`")?;
        let ty = self.type_expr()?;
        let mut end = ty.span;

        let optional = if self.peek().kind == TokenKind::Question {
            end = self.bump().span;
            true
        } else {
            false
        };

        // `check fn_name` — the field validator (RFD 0013), between the
        // `?` and `unique`. `check (` here belongs to the *next* table item
        // (a row check), so require `(` not follow — exactly the `unique (`
        // disambiguation. Inline SQL gets a targeted message (L-D).
        let check = if self.peek().kind == TokenKind::KwCheck
            && self.peek2().kind != TokenKind::LParen
        {
            let check_kw = self.bump().span;
            if let TokenKind::Str(_) = self.peek().kind {
                return Err(Diagnostic::new(
                    "L010",
                    "`check` references a validator fn by name (`check valid_x`); inline SQL is not allowed — declare `fn valid_x(...) -> bool { ... }`",
                    self.peek().span,
                ));
            }
            // An omitted validator name would swallow the next field's name —
            // guard the common case (a field line follows) at the `check`
            // keyword, mirroring the row-check path (L-D).
            if matches!(self.peek().kind, TokenKind::Ident(_))
                && self.peek2().kind == TokenKind::Colon
            {
                return Err(Diagnostic::new(
                    "L010",
                    "field check is missing its validator fn name: write `name: type check fn_name`",
                    check_kw,
                ));
            }
            let name = self.ident("a validator fn name")?;
            // `check fn(a, b)` — the function-call spelling of a row check
            // (a field check is never followed by `(`). Steer to the real
            // form (L-D).
            if self.peek().kind == TokenKind::LParen {
                return Err(Diagnostic::new(
                    "L010",
                    "a row check names its fields first, then the validator: `check (a, b) fn_name` — not a function call `check fn(a, b)`",
                    self.peek().span,
                ));
            }
            end = name.span;
            Some(name)
        } else {
            None
        };

        // `unique` here is the field modifier; `unique (` would be the next
        // table item (a group), so require that `(` does not follow.
        let unique =
            if self.peek().kind == TokenKind::KwUnique && self.peek2().kind != TokenKind::LParen {
                end = self.bump().span;
                true
            } else {
                false
            };

        let default = if self.peek().kind == TokenKind::Eq {
            self.bump();
            let d = self.default_expr()?;
            end = d.span();
            Some(d)
        } else {
            None
        };

        let on_delete = if self.peek().kind == TokenKind::KwOn {
            let on = self.bump().span;
            self.expect(TokenKind::KwDelete, "`delete`")?;
            let (kind, kspan) = match self.peek().kind {
                TokenKind::KwRestrict => (OnDeleteKind::Restrict, self.bump().span),
                TokenKind::KwCascade => (OnDeleteKind::Cascade, self.bump().span),
                TokenKind::KwSet => {
                    self.bump();
                    let end = self.expect(TokenKind::KwNull, "`null`")?.span;
                    (OnDeleteKind::SetNull, end)
                }
                _ => return Err(self.unexpected("`restrict`, `cascade`, or `set null`")),
            };
            end = kspan;
            Some(OnDeleteClause {
                kind,
                span: on.to(kspan),
            })
        } else {
            None
        };

        Ok(FieldDecl {
            doc: None,
            is_key,
            name,
            ty,
            optional,
            unique,
            check,
            default,
            on_delete,
            span: start.to(end),
        })
    }

    fn type_expr(&mut self) -> Result<TypeExpr, Diagnostic> {
        // A string literal at type position opens a closed-set type
        // (`"a" | "b"`) — the checker enforces its laws (RFD 0013).
        if let TokenKind::Str(_) = self.peek().kind {
            return self.set_type();
        }
        let tok = self.peek().clone();
        let kind = match &tok.kind {
            TokenKind::KwText => TypeExprKind::Text,
            TokenKind::KwInt => TypeExprKind::Int,
            TokenKind::KwFloat => TypeExprKind::Float,
            TokenKind::KwBool => TypeExprKind::Bool,
            TokenKind::KwTimestamp => TypeExprKind::Timestamp,
            TokenKind::KwUuid => TypeExprKind::Uuid,
            TokenKind::Ident(name) => TypeExprKind::Named(name.clone()),
            _ => return Err(self.unexpected("a type")),
        };
        self.bump();
        Ok(TypeExpr {
            kind,
            span: tok.span,
        })
    }

    // set_type = string { "|" string } — the singleton/dup/empty laws are
    // the checker's (E043), so the production admits one-or-more here.
    fn set_type(&mut self) -> Result<TypeExpr, Diagnostic> {
        let first = self.bump();
        let TokenKind::Str(value) = first.kind else {
            unreachable!("set_type entered on a string token")
        };
        let start = first.span;
        let mut end = first.span;
        let mut members = vec![SetMember {
            value,
            span: first.span,
        }];
        while self.peek().kind == TokenKind::Pipe {
            self.bump();
            let tok = self.bump();
            match tok.kind {
                TokenKind::Str(value) => {
                    end = tok.span;
                    members.push(SetMember {
                        value,
                        span: tok.span,
                    });
                }
                _ => {
                    return Err(Diagnostic::new(
                        "L010",
                        format!(
                            "expected another set member (a string literal), found {}",
                            tok.kind.describe()
                        ),
                        tok.span,
                    ));
                }
            }
        }
        Ok(TypeExpr {
            kind: TypeExprKind::Set(members),
            span: start.to(end),
        })
    }

    fn default_expr(&mut self) -> Result<DefaultExpr, Diagnostic> {
        let tok = self.peek().clone();
        let expr = match &tok.kind {
            TokenKind::KwAuto => DefaultExpr::Auto(tok.span),
            TokenKind::KwNow => DefaultExpr::Now(tok.span),
            TokenKind::KwMe => DefaultExpr::Me(tok.span),
            TokenKind::Str(s) => DefaultExpr::Lit(Lit::Str(s.clone(), tok.span)),
            TokenKind::Int(v) => DefaultExpr::Lit(Lit::Int(*v, tok.span)),
            TokenKind::Float(v) => DefaultExpr::Lit(Lit::Float(*v, tok.span)),
            TokenKind::KwTrue => DefaultExpr::Lit(Lit::Bool(true, tok.span)),
            TokenKind::KwFalse => DefaultExpr::Lit(Lit::Bool(false, tok.span)),
            _ => return Err(self.unexpected("`auto`, `now`, `me`, or a literal")),
        };
        self.bump();
        Ok(expr)
    }

    // seed_block = "seed" "{" { seed_stmt } "}"
    fn seed_block(&mut self) -> Result<SeedBlock, Diagnostic> {
        let start = self.expect(TokenKind::KwSeed, "`seed`")?.span;
        self.expect(TokenKind::LBrace, "`{`")?;
        let mut stmts = Vec::new();
        let end = loop {
            match self.peek().kind {
                TokenKind::RBrace => break self.bump().span,
                TokenKind::Eof => return Err(self.unexpected("`}`")),
                _ => stmts.push(self.seed_stmt()?),
            }
        };
        Ok(SeedBlock {
            stmts,
            span: start.to(end),
        })
    }

    // seed_stmt = [ident "="] ident "{" [seed_field {"," seed_field} [","]] "}"
    fn seed_stmt(&mut self) -> Result<SeedStmt, Diagnostic> {
        let first = self.ident("table name or binding")?;
        let (binding, table) = if self.peek().kind == TokenKind::Eq {
            self.bump();
            let table = self.ident("table name")?;
            (Some(first), table)
        } else {
            (None, first)
        };
        let start = binding.as_ref().map(|b| b.span).unwrap_or(table.span);
        self.expect(TokenKind::LBrace, "`{`")?;

        let mut fields = Vec::new();
        let end;
        loop {
            if self.peek().kind == TokenKind::RBrace {
                end = self.bump().span;
                break;
            }
            let name = self.ident("field name")?;
            self.expect(TokenKind::Colon, "`:`")?;
            let value = self.seed_value()?;
            fields.push((name, value));
            match self.peek().kind {
                TokenKind::Comma => {
                    self.bump();
                }
                TokenKind::RBrace => {}
                _ => return Err(self.unexpected("`,` or `}`")),
            }
        }

        Ok(SeedStmt {
            binding,
            table,
            fields,
            span: start.to(end),
        })
    }

    fn seed_value(&mut self) -> Result<SeedValue, Diagnostic> {
        let tok = self.peek().clone();
        // `file("./path")` — a seed-time asset load (RFD 0018). A soft keyword:
        // `file` is only special immediately before `(`, so a binding or field
        // named `file` is unaffected.
        if let TokenKind::Ident(name) = &tok.kind {
            if name == SOFT_KEYWORDS[0] && matches!(self.peek2().kind, TokenKind::LParen) {
                return self.seed_file(tok.span);
            }
        }
        let value = match &tok.kind {
            TokenKind::Str(s) => SeedValue::Lit(Lit::Str(s.clone(), tok.span)),
            TokenKind::Int(v) => SeedValue::Lit(Lit::Int(*v, tok.span)),
            TokenKind::Float(v) => SeedValue::Lit(Lit::Float(*v, tok.span)),
            TokenKind::KwTrue => SeedValue::Lit(Lit::Bool(true, tok.span)),
            TokenKind::KwFalse => SeedValue::Lit(Lit::Bool(false, tok.span)),
            TokenKind::Ident(name) => SeedValue::Binding(Ident {
                name: name.clone(),
                span: tok.span,
            }),
            _ => return Err(self.unexpected("a literal, a seed binding, or file(\"...\")")),
        };
        self.bump();
        Ok(value)
    }

    // file_value = "file" "(" str ")"
    fn seed_file(&mut self, start: Span) -> Result<SeedValue, Diagnostic> {
        self.bump(); // `file`
        self.expect(TokenKind::LParen, "`(` after `file`")?;
        let tok = self.peek().clone();
        let TokenKind::Str(path) = &tok.kind else {
            return Err(self.unexpected("a string path, e.g. file(\"./avatar.png\")"));
        };
        let path = path.clone();
        self.bump();
        let close = self.expect(TokenKind::RParen, "`)` to close `file(...)`")?;
        Ok(SeedValue::File {
            path,
            span: start.to(close.span),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    fn parse_ok(source: &str) -> File {
        parse(lex(source).unwrap()).unwrap()
    }

    fn parse_err(source: &str) -> Diagnostic {
        parse(lex(source).unwrap()).unwrap_err()
    }

    #[test]
    fn parses_full_table() {
        let file = parse_ok(
            "table post {\n\
               key id: uuid = auto\n\
               author: user on delete cascade\n\
               caption: text?\n\
               pinned: bool = false\n\
               created_at: timestamp = now\n\
             }",
        );
        assert_eq!(file.tables.len(), 1);
        let table = &file.tables[0];
        assert_eq!(table.name.name, "post");
        assert_eq!(table.items.len(), 5);
        let TableItem::Field(id) = &table.items[0] else {
            panic!("expected field");
        };
        assert!(id.is_key);
        assert!(matches!(id.default, Some(DefaultExpr::Auto(_))));
        let TableItem::Field(author) = &table.items[1] else {
            panic!("expected field");
        };
        assert!(matches!(&author.ty.kind, TypeExprKind::Named(n) if n == "user"));
        assert_eq!(
            author.on_delete.as_ref().map(|c| c.kind),
            Some(OnDeleteKind::Cascade)
        );
        let TableItem::Field(caption) = &table.items[2] else {
            panic!("expected field");
        };
        assert!(caption.optional);
    }

    #[test]
    fn parses_on_delete_set_null() {
        let file = parse_ok(
            "table comment {\n\
               key id: uuid = auto\n\
               parent: comment? on delete set null\n\
             }",
        );
        let TableItem::Field(parent) = &file.tables[0].items[1] else {
            panic!("expected field");
        };
        assert_eq!(
            parent.on_delete.as_ref().map(|c| c.kind),
            Some(OnDeleteKind::SetNull)
        );
        // `set` must be followed by `null`
        let d = parse_err("table t { key id: uuid = auto\n a: t? on delete set }");
        assert!(d.message.contains("`null`"), "{}", d.message);
    }

    #[test]
    fn parses_closed_set_types() {
        let file = parse_ok(
            "table media {\n\
               key id: uuid = auto\n\
               kind: \"image\" | \"video\"\n\
               status: \"pending\" | \"ready\" | \"failed\" = \"pending\"\n\
             }",
        );
        let table = &file.tables[0];
        let TableItem::Field(kind) = &table.items[1] else {
            panic!("expected field");
        };
        let TypeExprKind::Set(members) = &kind.ty.kind else {
            panic!("expected a set type");
        };
        assert_eq!(members.len(), 2);
        assert_eq!(members[0].value, "image");
        assert_eq!(members[1].value, "video");
        // a set field still takes a default and it round-trips as a string
        let TableItem::Field(status) = &table.items[2] else {
            panic!("expected field");
        };
        assert!(
            matches!(&status.default, Some(DefaultExpr::Lit(Lit::Str(s, _))) if s == "pending")
        );
        // the singleton case is a *parse*, deferred to the checker (E043)
        let one = parse_ok("table t { key id: uuid = auto\n s: \"only\" }");
        let TableItem::Field(s) = &one.tables[0].items[1] else {
            panic!("expected field");
        };
        assert!(matches!(&s.ty.kind, TypeExprKind::Set(m) if m.len() == 1));
        // a dangling `|` wants another member
        let d = parse_err("table t { key id: uuid = auto\n s: \"a\" | }");
        assert!(d.message.contains("set member"), "{}", d.message);
    }

    #[test]
    fn parses_field_and_row_checks() {
        let file = parse_ok(
            "table user {\n\
               key id: uuid = auto\n\
               username: text check valid_username unique\n\
             }\n\
             table follow {\n\
               key (follower, target)\n\
               follower: user\n\
               target: user\n\
               check (follower, target) distinct_pair\n\
             }",
        );
        let TableItem::Field(username) = &file.tables[0].items[1] else {
            panic!("expected field");
        };
        assert!(matches!(&username.check, Some(i) if i.name == "valid_username"));
        assert!(username.unique); // check comes before unique
        let TableItem::Check {
            fields, fn_name, ..
        } = &file.tables[1].items[3]
        else {
            panic!("expected a row check");
        };
        assert_eq!(fields.len(), 2);
        assert_eq!(fn_name.name, "distinct_pair");
    }

    #[test]
    fn check_misspellings_get_targeted_messages() {
        // parenthesized field check → steered to the bare-name / row-check forms
        let d = parse_err("table t { key id: uuid = auto\n s: text check(v) }");
        assert!(d.message.contains("bare fn name"), "{}", d.message);
        // inline SQL in check position → steered to declare a fn
        let d = parse_err("table t { key id: uuid = auto\n s: text check \"len > 0\" }");
        assert!(d.message.contains("validator fn"), "{}", d.message);
        // the fn-call spelling at item position
        let d = parse_err("table t { key id: uuid = auto\n a: t\n check distinct(a) }");
        assert!(d.message.contains("row check"), "{}", d.message);
        // an omitted validator swallows the next field → targeted message
        let d =
            parse_err("table t { key (a, b)\n a: t\n b: t\n check (a, b)\n c: timestamp = now }");
        assert!(d.message.contains("missing its validator"), "{}", d.message);
        // the misordered `unique check`
        let d = parse_err("table t { key id: uuid = auto\n s: text unique check v }");
        assert!(d.message.contains("field check"), "{}", d.message);
        // a field check with the validator name omitted → targeted message
        // at the `check`, not a swallow of the next field
        let d = parse_err("table t { key id: uuid = auto\n username: text check\n pw: text }");
        assert!(d.message.contains("missing its validator"), "{}", d.message);
    }

    #[test]
    fn parses_composite_key_and_unique_group() {
        let file = parse_ok(
            "table follow {\n\
               key (follower, target)\n\
               follower: user\n\
               target: user\n\
               unique (follower, target)\n\
             }",
        );
        let table = &file.tables[0];
        assert!(matches!(&table.items[0], TableItem::Key { fields, .. } if fields.len() == 2));
        assert!(matches!(&table.items[3], TableItem::Unique { fields, .. } if fields.len() == 2));
    }

    #[test]
    fn distinguishes_field_unique_from_group() {
        let file = parse_ok(
            "table user {\n\
               key id: uuid = auto\n\
               username: text unique\n\
               unique (id, username)\n\
             }",
        );
        let table = &file.tables[0];
        let TableItem::Field(username) = &table.items[1] else {
            panic!("expected field");
        };
        assert!(username.unique);
        assert!(matches!(&table.items[2], TableItem::Unique { .. }));
    }

    #[test]
    fn parses_seed_with_bindings() {
        let file = parse_ok(
            "seed {\n\
               maya = user { username: \"maya\" }\n\
               post { author: maya, caption: \"hi\", }\n\
             }",
        );
        let seed = &file.seeds[0];
        assert_eq!(seed.stmts.len(), 2);
        assert_eq!(seed.stmts[0].binding.as_ref().unwrap().name, "maya");
        assert!(seed.stmts[1].binding.is_none());
        assert!(matches!(&seed.stmts[1].fields[0].1, SeedValue::Binding(b) if b.name == "maya"));
    }

    #[test]
    fn parses_full_fn() {
        let file = parse_ok(
            "fn rename_user(user: user, name: text, note: text?) -> user ! user_username_taken | not_found {\n\
               unchecked sql(\"\"\"\n\
                 UPDATE user SET username = :name WHERE id = :user RETURNING *\n\
               \"\"\")\n\
             }",
        );
        assert_eq!(file.fns.len(), 1);
        let f = &file.fns[0];
        assert_eq!(f.name.name, "rename_user");
        assert_eq!(f.params.len(), 3);
        assert!(matches!(&f.params[0].ty.kind, TypeExprKind::Named(n) if n == "user"));
        assert!(!f.params[1].optional);
        assert!(f.params[2].optional);
        assert_eq!(f.ret.arity, RetArity::One);
        assert!(matches!(&f.ret.target, RetTarget::Named(n) if n.name == "user"));
        assert_eq!(f.errors.len(), 2);
        assert_eq!(f.errors[1].name, "not_found");
        assert_eq!(f.body.len(), 1);
        assert!(f.body[0].sql.contains("RETURNING *"));
    }

    #[test]
    fn parses_error_declarations() {
        let file = parse_ok(
            "/// The account is not visible to this actor.\n\
             error account_private\n\
             error request_expired",
        );
        assert_eq!(file.errors.len(), 2);
        assert_eq!(file.errors[0].name.name, "account_private");
        assert_eq!(
            file.errors[0].doc.as_deref(),
            Some("The account is not visible to this actor.")
        );
        assert_eq!(file.errors[1].name.name, "request_expired");
        assert!(file.errors[1].doc.is_none());
    }

    #[test]
    fn parses_polarity_markers() {
        let file = parse_ok(
            "mut fn rename(user: user, name: text) -> user { unchecked sql(\"S\") }\n\
             fn find(name: text) -> user? { unchecked sql(\"S\") }",
        );
        assert!(file.fns[0].mutates);
        assert!(!file.fns[1].mutates);
        // `mut` marks fns only
        let d = parse_err("mut table user { key id: uuid = auto }");
        assert_eq!(d.code, "L010");
        assert!(d.message.contains("only a fn"), "{}", d.message);
    }

    #[test]
    fn parses_auth_table_anchor() {
        let file = parse_ok(
            "auth table user { key id: uuid = auto }\n\
             table post { key id: uuid = auto }",
        );
        assert!(file.tables[0].auth.is_some());
        assert!(file.tables[1].auth.is_none());
        // `auth` marks tables only
        let d = parse_err("auth fn f() -> t { unchecked sql(\"x\") }");
        assert_eq!(d.code, "L010");
        assert!(d.message.contains("only a table"), "{}", d.message);
    }

    #[test]
    fn parses_multi_statement_bodies() {
        let file = parse_ok(
            "fn approve(request: uuid) -> follow_request {\n\
               unchecked sql(\"UPDATE follow_request SET status = 'approved' WHERE id = :request\")\n\
               unchecked sql(\"SELECT * FROM follow_request WHERE id = :request\")\n\
             }",
        );
        let f = &file.fns[0];
        assert_eq!(f.body.len(), 2);
        assert!(f.body[0].sql.starts_with("UPDATE"));
        assert!(f.body[1].sql.starts_with("SELECT"));
        // every escape carries its own marker: a bare second sql(...) fails
        let d = parse_err(
            "fn f() -> t {\n\
               unchecked sql(\"UPDATE x SET a = 1\")\n\
               sql(\"SELECT * FROM x\")\n\
             }",
        );
        assert_eq!(d.code, "L010");
        assert!(d.message.contains("unchecked"));
    }

    #[test]
    fn parses_fn_arities_and_zero_params() {
        let file = parse_ok(
            "fn find(name: text) -> user? { unchecked sql(\"x\") }\n\
             fn recent(n: int) -> [post] { unchecked sql(\"x\") }\n\
             fn hello() -> greeting { unchecked sql(\"x\") }\n\
             fn trailing(a: int,) -> t { unchecked sql(\"x\") }",
        );
        assert_eq!(file.fns[0].ret.arity, RetArity::Maybe);
        assert_eq!(file.fns[1].ret.arity, RetArity::Many);
        assert!(matches!(&file.fns[1].ret.target, RetTarget::Named(n) if n.name == "post"));
        assert!(file.fns[2].params.is_empty());
        assert!(file.fns[2].errors.is_empty());
        assert_eq!(file.fns[3].params.len(), 1); // trailing comma
    }

    #[test]
    fn parses_record_declaration() {
        let file = parse_ok("record stats { posts: int\n latest: timestamp? }");
        assert_eq!(file.records.len(), 1);
        assert_eq!(file.records[0].name.name, "stats");
        assert_eq!(file.records[0].items.len(), 2);
    }

    #[test]
    fn sql_stays_an_identifier_outside_fn_bodies() {
        let file = parse_ok("table sql { key id: uuid = auto\n sql: text }");
        assert_eq!(file.tables[0].name.name, "sql");
    }

    #[test]
    fn unchecked_stays_an_identifier_outside_fn_bodies() {
        let file = parse_ok("table unchecked { key id: uuid = auto\n unchecked: bool = false }");
        assert_eq!(file.tables[0].name.name, "unchecked");
    }

    #[test]
    fn parses_scalar_returns() {
        let file = parse_ok(
            "fn count() -> int { unchecked sql(\"x\") }\n\
             fn latest() -> timestamp? { unchecked sql(\"x\") }\n\
             fn names() -> [text] { unchecked sql(\"x\") }",
        );
        assert!(matches!(
            &file.fns[0].ret.target,
            RetTarget::Scalar(TypeExprKind::Int, _)
        ));
        assert_eq!(file.fns[1].ret.arity, RetArity::Maybe);
        assert!(matches!(
            &file.fns[1].ret.target,
            RetTarget::Scalar(TypeExprKind::Timestamp, _)
        ));
        assert_eq!(file.fns[2].ret.arity, RetArity::Many);
        assert!(matches!(
            &file.fns[2].ret.target,
            RetTarget::Scalar(TypeExprKind::Text, _)
        ));
    }

    #[test]
    fn rejects_malformed_fns() {
        // missing arrow
        assert_eq!(
            parse_err("fn f() user { unchecked sql(\"x\") }").code,
            "L010"
        );
        // body must open with the marker
        let d = parse_err("fn f() -> t { nosql(\"x\") }");
        assert!(d.message.contains("expected `unchecked`"), "{}", d.message);
        // an unmarked escape gets the acknowledgment guidance (RFD 0011)
        let d = parse_err("fn f() -> t { sql(\"x\") }");
        assert!(
            d.message.contains("unchecked sql") && d.message.contains("cannot verify"),
            "{}",
            d.message
        );
        // the marker alone is not a body
        let d = parse_err("fn f() -> t { unchecked nosql(\"x\") }");
        assert!(d.message.contains("expected `sql`"), "{}", d.message);
        // body must carry a string
        assert_eq!(parse_err("fn f() -> t { unchecked sql(42) }").code, "L010");
        // unclosed body
        assert_eq!(parse_err("fn f() -> t { unchecked sql(\"x\")").code, "L010");
    }

    #[test]
    fn rejects_single_field_group() {
        // a one-field group must be written as the field modifier
        let d = parse_err("table t { key (a) a: int }");
        assert_eq!(d.code, "L010");
    }

    #[test]
    fn rejects_stray_top_level_token() {
        let d = parse_err("42");
        assert_eq!(d.code, "L010");
    }

    #[test]
    fn rejects_unclosed_table() {
        let d = parse_err("table t { a: int");
        assert_eq!(d.code, "L010");
    }

    #[test]
    fn parses_doc_comments() {
        let file = parse_ok(
            "//! the contract\n\
             //! second line\n\
             \n\
             /// a person\n\
             auth table user {\n\
               key id: uuid = auto\n\
               /// the handle\n\
               username: text unique\n\
             }\n\
             /// a wire shape\n\
             record stats { /// how many\n posts: int }\n\
             /// account cannot be viewed\n\
             error account_private\n\
             /// rename someone\n\
             mut fn rename(\n\
               /// the target\n\
               user: user,\n\
               /// the new name\n\
               name: text,\n\
             ) -> user { unchecked sql(\"S\") }",
        );
        // contract doc = the joined `//!` preamble
        assert_eq!(file.doc.as_deref(), Some("the contract\nsecond line"));
        // table doc, attached across the `auth` prefix
        assert_eq!(file.tables[0].doc.as_deref(), Some("a person"));
        // field doc (items[0] is the key field, items[1] is username)
        let TableItem::Field(username) = &file.tables[0].items[1] else {
            panic!("expected field");
        };
        assert_eq!(username.doc.as_deref(), Some("the handle"));
        // record + record-field docs
        assert_eq!(file.records[0].doc.as_deref(), Some("a wire shape"));
        let TableItem::Field(posts) = &file.records[0].items[0] else {
            panic!("expected field");
        };
        assert_eq!(posts.doc.as_deref(), Some("how many"));
        // product-error declaration doc
        assert_eq!(
            file.errors[0].doc.as_deref(),
            Some("account cannot be viewed")
        );
        // fn doc across the `mut` prefix, plus per-parameter docs
        assert_eq!(file.fns[0].doc.as_deref(), Some("rename someone"));
        assert_eq!(file.fns[0].params[0].doc.as_deref(), Some("the target"));
        assert_eq!(file.fns[0].params[1].doc.as_deref(), Some("the new name"));
    }

    #[test]
    fn doc_run_joins_across_blank_and_ordinary_comments() {
        let file = parse_ok(
            "/// line one\n\
             // an ordinary comment does not break the run\n\
             \n\
             /// line two\n\
             table t { key id: uuid = auto }",
        );
        assert_eq!(file.tables[0].doc.as_deref(), Some("line one\nline two"));
    }

    #[test]
    fn empty_doc_is_a_no_op() {
        // a lone `///` attaches nothing and is not dangling
        let file = parse_ok("///\ntable t { key id: uuid = auto }");
        assert_eq!(file.tables[0].doc, None);
        assert!(file.doc.is_none());
    }

    #[test]
    fn rejects_dangling_doc() {
        // before the closing brace
        assert_eq!(
            parse_err("table t { key id: uuid = auto\n /// nothing\n }").code,
            "L011"
        );
        // on a non-documentable row item (a unique group)
        assert_eq!(
            parse_err("table t { key (a, b)\n a: t\n b: t\n /// bad\n unique (a, b) }").code,
            "L011"
        );
        // inside a fn body
        assert_eq!(
            parse_err("fn f() -> t {\n /// bad\n unchecked sql(\"S\") }").code,
            "L011"
        );
        // before a seed block (seeds are not documentable)
        assert_eq!(parse_err("/// bad\n seed { }").code, "L011");
        // trailing at end of file
        assert_eq!(
            parse_err("table t { key id: uuid = auto }\n /// trailing").code,
            "L011"
        );
    }

    #[test]
    fn rejects_misplaced_inner_doc() {
        // `//!` after the first declaration
        assert_eq!(
            parse_err("table t { key id: uuid = auto }\n //! too late").code,
            "L012"
        );
        // `//!` inside a table body
        assert_eq!(
            parse_err("table t { //! nope\n key id: uuid = auto }").code,
            "L012"
        );
    }
}

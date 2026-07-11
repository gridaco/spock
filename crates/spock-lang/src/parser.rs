//! Recursive-descent parser (docs/spec/v0.md §3). Fail-fast: the first
//! syntax error aborts the parse; the checker (many-error) runs after.

use crate::ast::*;
use crate::diag::Diagnostic;
use crate::lexer::{Token, TokenKind};
use crate::span::Span;

pub fn parse(tokens: Vec<Token>) -> Result<File, Diagnostic> {
    Parser { tokens, pos: 0 }.file()
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
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

    // file = { table_decl | record_decl | fn_decl | seed_block }
    fn file(&mut self) -> Result<File, Diagnostic> {
        let mut tables = Vec::new();
        let mut records = Vec::new();
        let mut fns = Vec::new();
        let mut seeds = Vec::new();
        loop {
            match self.peek().kind {
                TokenKind::KwTable => tables.push(self.table_decl()?),
                TokenKind::KwRecord => records.push(self.record_decl()?),
                TokenKind::KwFn => fns.push(self.fn_decl(false)?),
                TokenKind::KwMut => {
                    let start = self.bump().span;
                    if self.peek().kind != TokenKind::KwFn {
                        return Err(self.unexpected("`fn` (only a fn can be `mut`)"));
                    }
                    let mut decl = self.fn_decl(true)?;
                    decl.span = start.to(decl.span);
                    fns.push(decl);
                }
                TokenKind::KwSeed => seeds.push(self.seed_block()?),
                TokenKind::Eof => break,
                _ => {
                    return Err(self.unexpected("`table`, `record`, `fn`, `mut fn`, or `seed`"));
                }
            }
        }
        Ok(File {
            tables,
            records,
            fns,
            seeds,
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
                _ => items.push(self.table_item()?),
            }
        };
        Ok(TableDecl {
            name,
            items,
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
                _ => items.push(self.table_item()?),
            }
        };
        Ok(RecordDecl {
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
        if marker.name == "sql" {
            return Err(Diagnostic::new(
                "L010",
                "a fn body must acknowledge the escape: write `unchecked sql(\"...\")` — the checker cannot verify SQL (RFD 0011)",
                marker.span,
            ));
        }
        if marker.name != "unchecked" {
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
        if escape.name != "sql" {
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
            _ => Ok(TableItem::Field(self.field_decl()?)),
        }
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
            is_key,
            name,
            ty,
            optional,
            unique,
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
        let mut members = vec![SetMember { value, span: first.span }];
        while self.peek().kind == TokenKind::Pipe {
            self.bump();
            let tok = self.bump();
            match tok.kind {
                TokenKind::Str(value) => {
                    end = tok.span;
                    members.push(SetMember { value, span: tok.span });
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
            TokenKind::Str(s) => DefaultExpr::Lit(Lit::Str(s.clone(), tok.span)),
            TokenKind::Int(v) => DefaultExpr::Lit(Lit::Int(*v, tok.span)),
            TokenKind::Float(v) => DefaultExpr::Lit(Lit::Float(*v, tok.span)),
            TokenKind::KwTrue => DefaultExpr::Lit(Lit::Bool(true, tok.span)),
            TokenKind::KwFalse => DefaultExpr::Lit(Lit::Bool(false, tok.span)),
            _ => return Err(self.unexpected("`auto`, `now`, or a literal")),
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
            _ => return Err(self.unexpected("a literal or a seed binding")),
        };
        self.bump();
        Ok(value)
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
        assert!(matches!(&status.default, Some(DefaultExpr::Lit(Lit::Str(s, _))) if s == "pending"));
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
        assert_eq!(parse_err("fn f() user { unchecked sql(\"x\") }").code, "L010");
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
}

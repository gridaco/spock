//! Checker: lowers the AST to the contract IR, validating everything the
//! spec requires (docs/spec/v0.md §4). Reports as many diagnostics as it
//! can find; the contract is produced only when there are none.

use std::collections::{BTreeMap, HashMap, HashSet};

use crate::ast;
use crate::diag::Diagnostic;
use crate::ir::*;
use crate::span::Span;

pub fn check(file: &ast::File) -> Result<Contract, Vec<Diagnostic>> {
    let mut checker = Checker::default();
    let contract = checker.run(file);
    if checker.diags.is_empty() {
        Ok(contract)
    } else {
        Err(checker.diags)
    }
}

#[derive(Default)]
struct Checker {
    diags: Vec<Diagnostic>,
}

impl Checker {
    fn error(&mut self, code: &'static str, message: impl Into<String>, span: Span) {
        self.diags.push(Diagnostic::new(code, message, span));
    }

    fn run(&mut self, file: &ast::File) -> Contract {
        // Table namespace (E001).
        let mut names: HashMap<&str, Span> = HashMap::new();
        for table in &file.tables {
            if names.contains_key(table.name.name.as_str()) {
                self.error(
                    "E001",
                    format!("duplicate table name `{}`", table.name.name),
                    table.name.span,
                );
            } else {
                names.insert(&table.name.name, table.name.span);
            }
        }
        let table_names: HashSet<&str> = names.keys().copied().collect();

        // Phase A: lower each table in isolation.
        let mut tables: Vec<Table> = Vec::new();
        let mut spans: HashMap<String, Span> = HashMap::new();
        for decl in &file.tables {
            if let Some(table) = self.lower_table(decl, &table_names) {
                spans.insert(table.name.clone(), decl.name.span);
                tables.push(table);
            }
        }

        // Phase B: cross-table validation (needs every table's key resolved).
        self.check_ref_targets(&tables, &spans, file);
        self.check_key_cycles(&tables, &spans);

        // Derived errors (§6.1) — needs inbound-ref knowledge, so done last.
        let derived: Vec<Vec<DerivedError>> =
            tables.iter().map(|t| derive_errors(t, &tables)).collect();
        for (table, errors) in tables.iter_mut().zip(derived) {
            table.errors = errors;
        }

        // Seed (§4 E020–E028), against the lowered tables.
        let seed = self.check_seed(file, &tables);

        // Phase C: records and fns (§4 E030–E039). Runs after derived
        // errors are attached — the `!` clause validates against them.
        let records = self.check_records(file, &table_names);
        let fns = self.check_fns(file, &tables, &records, &table_names);

        Contract {
            spock: "v0".into(),
            tables,
            records,
            fns,
            seed,
        }
    }

    /// Records (§3): named wire shapes. Scalar fields only; table-only
    /// syntax is rejected with the offending span (E033); records share
    /// the type namespace with tables (E031).
    fn check_records(&mut self, file: &ast::File, table_names: &HashSet<&str>) -> Vec<Record> {
        let mut names: HashMap<&str, Span> = HashMap::new();
        let mut records = Vec::new();
        for decl in &file.records {
            let rname = decl.name.name.as_str();
            if names.contains_key(rname) {
                self.error(
                    "E030",
                    format!("duplicate record name `{rname}`"),
                    decl.name.span,
                );
                continue;
            }
            names.insert(rname, decl.name.span);
            if table_names.contains(rname) {
                self.error(
                    "E031",
                    format!("record `{rname}` collides with a table of the same name (tables and records share one type namespace)"),
                    decl.name.span,
                );
                continue;
            }

            let mut fields: Vec<RecordField> = Vec::new();
            let mut field_spans: HashMap<String, Span> = HashMap::new();
            let mut declared = 0usize;
            for item in &decl.items {
                let f = match item {
                    ast::TableItem::Key { span, .. } | ast::TableItem::Unique { span, .. } => {
                        self.error(
                            "E033",
                            format!("record `{rname}` cannot declare a key or unique group (records are wire shapes, not storage)"),
                            *span,
                        );
                        continue;
                    }
                    ast::TableItem::Field(f) => f,
                };
                declared += 1;
                if field_spans.contains_key(&f.name.name) {
                    self.error(
                        "E002",
                        format!("duplicate field `{}` in record `{rname}`", f.name.name),
                        f.name.span,
                    );
                    continue;
                }
                field_spans.insert(f.name.name.clone(), f.name.span);
                // table-only modifiers (anti-cascade: report, keep lowering)
                if f.is_key {
                    self.error(
                        "E033",
                        format!("`key` on record field `{}` (records have no keys)", f.name.name),
                        f.name.span,
                    );
                }
                if f.unique {
                    self.error(
                        "E033",
                        format!("`unique` on record field `{}` (records are not stored)", f.name.name),
                        f.name.span,
                    );
                }
                if let Some(d) = &f.default {
                    self.error(
                        "E033",
                        format!("default on record field `{}` (records are read shapes)", f.name.name),
                        d.span(),
                    );
                }
                if let Some(c) = &f.on_delete {
                    self.error(
                        "E033",
                        format!("`on delete` on record field `{}` (records hold no references)", f.name.name),
                        c.span,
                    );
                }
                let ty = match &f.ty.kind {
                    ast::TypeExprKind::Text => Type::Text,
                    ast::TypeExprKind::Int => Type::Int,
                    ast::TypeExprKind::Float => Type::Float,
                    ast::TypeExprKind::Bool => Type::Bool,
                    ast::TypeExprKind::Timestamp => Type::Timestamp,
                    ast::TypeExprKind::Uuid => Type::Uuid,
                    ast::TypeExprKind::Named(other) => {
                        self.error(
                            "E034",
                            format!("record field `{}` must be a builtin scalar; `{other}` is not permitted (SQL result columns are scalars)", f.name.name),
                            f.ty.span,
                        );
                        continue;
                    }
                    ast::TypeExprKind::Set(_) => {
                        self.error(
                            "E034",
                            format!("record field `{}` must be a builtin scalar; a closed-set type is not permitted (records are wire read shapes)", f.name.name),
                            f.ty.span,
                        );
                        continue;
                    }
                };
                fields.push(RecordField {
                    name: f.name.name.clone(),
                    ty,
                    optional: f.optional,
                });
            }
            if declared == 0 {
                self.error(
                    "E032",
                    format!("record `{rname}` has no fields"),
                    decl.name.span,
                );
                continue;
            }
            records.push(Record {
                name: decl.name.name.clone(),
                fields,
            });
        }
        records
    }

    /// Fns (§3, §7.4): the contract half. The SQL body is deliberately
    /// not inspected here — it is opaque to the language and validated by
    /// the engine at load.
    fn check_fns(
        &mut self,
        file: &ast::File,
        tables: &[Table],
        records: &[Record],
        table_names: &HashSet<&str>,
    ) -> Vec<FnDef> {
        // the `!` clause vocabulary: every derived code + the reserved five
        let mut vocabulary: HashSet<&str> = tables
            .iter()
            .flat_map(|t| t.errors.iter().map(|e| e.code.as_str()))
            .collect();
        vocabulary.extend(RESERVED_CODES);
        let record_names: HashSet<&str> = records.iter().map(|r| r.name.as_str()).collect();

        let mut names: HashMap<&str, Span> = HashMap::new();
        let mut fns = Vec::new();
        for decl in &file.fns {
            let fname = decl.name.name.as_str();
            if names.contains_key(fname) {
                self.error(
                    "E035",
                    format!("duplicate fn name `{fname}`"),
                    decl.name.span,
                );
                continue;
            }
            names.insert(fname, decl.name.span);

            let mut params: Vec<FnParam> = Vec::new();
            let mut param_spans: HashMap<&str, Span> = HashMap::new();
            for p in &decl.params {
                if param_spans.contains_key(p.name.name.as_str()) {
                    self.error(
                        "E038",
                        format!("duplicate parameter `{}` in fn `{fname}`", p.name.name),
                        p.name.span,
                    );
                    continue;
                }
                param_spans.insert(&p.name.name, p.name.span);
                let ty = match &p.ty.kind {
                    ast::TypeExprKind::Text => Type::Text,
                    ast::TypeExprKind::Int => Type::Int,
                    ast::TypeExprKind::Float => Type::Float,
                    ast::TypeExprKind::Bool => Type::Bool,
                    ast::TypeExprKind::Timestamp => Type::Timestamp,
                    ast::TypeExprKind::Uuid => Type::Uuid,
                    ast::TypeExprKind::Named(t) => {
                        if record_names.contains(t.as_str()) {
                            self.error(
                                "E036",
                                format!("fn parameter `{}` cannot be a record (`{t}`); records are return shapes in v0", p.name.name),
                                p.ty.span,
                            );
                            continue;
                        }
                        if !table_names.contains(t.as_str()) {
                            self.error(
                                "E036",
                                format!("unknown parameter type `{t}` (not a builtin, not a declared table)"),
                                p.ty.span,
                            );
                            continue;
                        }
                        // a param bound to a table binds ONE key scalar —
                        // composite-key targets have no such scalar
                        if tables.iter().any(|tb| tb.name == *t && tb.key.len() > 1) {
                            self.error(
                                "E036",
                                format!("fn parameter `{}` references `{t}`, whose key is composite; parameters bind a single key scalar", p.name.name),
                                p.ty.span,
                            );
                            continue;
                        }
                        Type::Ref {
                            table: t.clone(),
                            // inert: params delete nothing
                            on_delete: OnDelete::Restrict,
                        }
                    }
                    ast::TypeExprKind::Set(_) => {
                        self.error(
                            "E036",
                            format!("fn parameter `{}` cannot be a closed-set type; a set type is a table-field type in v0 (RFD 0013)", p.name.name),
                            p.ty.span,
                        );
                        continue;
                    }
                };
                params.push(FnParam {
                    name: p.name.name.clone(),
                    ty,
                    optional: p.optional,
                });
            }

            let (of, scalar) = match &decl.ret.target {
                ast::RetTarget::Named(ident) => {
                    let of = ident.name.as_str();
                    if !table_names.contains(of) && !record_names.contains(of) {
                        self.error(
                            "E037",
                            format!("fn `{fname}` returns unknown shape `{of}` (not a builtin scalar, declared table, or record)"),
                            ident.span,
                        );
                        continue;
                    }
                    (of.to_string(), false)
                }
                ast::RetTarget::Scalar(kind, _) => {
                    let name = match kind {
                        ast::TypeExprKind::Text => "text",
                        ast::TypeExprKind::Int => "int",
                        ast::TypeExprKind::Float => "float",
                        ast::TypeExprKind::Bool => "bool",
                        ast::TypeExprKind::Timestamp => "timestamp",
                        ast::TypeExprKind::Uuid => "uuid",
                        ast::TypeExprKind::Named(_) | ast::TypeExprKind::Set(_) => {
                            unreachable!("parser never scalars a name or set")
                        }
                    };
                    (name.to_string(), true)
                }
            };

            // the `!` clause has two populations (RFD 0012 §2.3): a code
            // in the vocabulary is a *reference* to a derived or reserved
            // code; a code in neither is a *refusal minted by this fn* —
            // raised from the body via spock_refuse(), routed at runtime.
            // (The cost: a misspelled derived code now mints instead of
            // erroring — dead metadata, never wrong behavior, since the
            // real constraint still routes to the true code.)
            let mut errors: Vec<String> = Vec::new();
            let mut refusals: Vec<String> = Vec::new();
            for code in &decl.errors {
                if errors.contains(&code.name) {
                    self.error(
                        "E039",
                        format!("duplicate error code `{}` in the `!` clause", code.name),
                        code.span,
                    );
                    continue;
                }
                if !vocabulary.contains(code.name.as_str()) {
                    refusals.push(code.name.clone());
                }
                errors.push(code.name.clone());
            }

            fns.push(FnDef {
                name: decl.name.name.clone(),
                readonly: !decl.mutates,
                params,
                returns: FnReturn {
                    arity: match decl.ret.arity {
                        ast::RetArity::One => FnArity::One,
                        ast::RetArity::Maybe => FnArity::Maybe,
                        ast::RetArity::Many => FnArity::Many,
                    },
                    of,
                    scalar,
                },
                errors,
                refusals,
                sql: decl.body.iter().map(|e| e.sql.clone()).collect(),
            });
        }
        fns
    }

    fn lower_table(&mut self, decl: &ast::TableDecl, table_names: &HashSet<&str>) -> Option<Table> {
        let mut fields: Vec<Field> = Vec::new();
        let mut field_spans: HashMap<String, Span> = HashMap::new();
        let mut inline_keys: Vec<(String, Span)> = Vec::new();
        let mut composite_keys: Vec<(&Vec<ast::Ident>, Span)> = Vec::new();
        let mut unique_groups: Vec<&Vec<ast::Ident>> = Vec::new();
        // Declared field count, even where lowering failed — E012 must not
        // cascade onto a field whose type had its own diagnostic.
        let mut declared_fields = 0usize;

        for item in &decl.items {
            match item {
                ast::TableItem::Field(f) => {
                    declared_fields += 1;
                    if field_spans.contains_key(&f.name.name) {
                        self.error(
                            "E002",
                            format!(
                                "duplicate field `{}` in table `{}`",
                                f.name.name, decl.name.name
                            ),
                            f.name.span,
                        );
                        continue;
                    }
                    field_spans.insert(f.name.name.clone(), f.name.span);
                    if f.is_key {
                        inline_keys.push((f.name.name.clone(), f.name.span));
                    }
                    if let Some(field) = self.lower_field(f, table_names) {
                        fields.push(field);
                    }
                }
                ast::TableItem::Key { fields, span } => composite_keys.push((fields, *span)),
                ast::TableItem::Unique { fields, .. } => unique_groups.push(fields),
            }
        }

        if declared_fields == 0 {
            self.error(
                "E012",
                format!("table `{}` has no fields", decl.name.name),
                decl.name.span,
            );
            return None;
        }

        // Exactly one key (E005/E006).
        let key_count = inline_keys.len() + composite_keys.len();
        if key_count == 0 {
            self.error(
                "E005",
                format!("table `{}` declares no key", decl.name.name),
                decl.name.span,
            );
        } else if key_count > 1 {
            let span = inline_keys
                .get(1)
                .map(|(_, s)| *s)
                .or_else(|| composite_keys.last().map(|(_, s)| *s))
                .unwrap_or(decl.name.span);
            self.error(
                "E006",
                format!("table `{}` declares more than one key", decl.name.name),
                span,
            );
        }

        let key: Vec<String> = if let Some((group, span)) = composite_keys.first() {
            self.validated_group(group, *span, &fields, "composite key", "E007")
        } else if let Some((name, _)) = inline_keys.first() {
            vec![name.clone()]
        } else {
            // No key: synthesize nothing; diagnostics already emitted.
            Vec::new()
        };

        // Key fields must not be optional (E008), nor a closed-set type
        // (E043, RFD 0013 L-B): forbidding a set key keeps `value_type`
        // from ever yielding a set through a reference, so a validator's
        // ref param always bottoms out at a builtin scalar.
        for name in &key {
            if let Some(field) = fields.iter().find(|f| &f.name == name) {
                if field.optional {
                    self.error(
                        "E008",
                        format!("key field `{name}` cannot be optional"),
                        field_spans[name],
                    );
                }
                if matches!(field.ty, Type::Set { .. }) {
                    self.error(
                        "E043",
                        format!("key field `{name}` cannot be a closed-set type"),
                        field_spans[name],
                    );
                }
            }
        }

        let uniques: Vec<Vec<String>> = unique_groups
            .iter()
            .map(|group| {
                let span = group.first().map(|i| i.span).unwrap_or(decl.name.span);
                self.validated_group(group, span, &fields, "unique group", "E011")
            })
            .filter(|g| !g.is_empty())
            .collect();

        Some(Table {
            name: decl.name.name.clone(),
            key,
            fields,
            uniques,
            errors: Vec::new(), // filled in phase B
        })
    }

    /// Validate a composite key / unique group: fields exist (`missing_code`),
    /// no duplicates (E014).
    fn validated_group(
        &mut self,
        group: &[ast::Ident],
        span: Span,
        fields: &[Field],
        what: &str,
        missing_code: &'static str,
    ) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut out = Vec::new();
        for ident in group {
            if fields.iter().all(|f| f.name != ident.name) {
                self.error(
                    missing_code,
                    format!("{what} names unknown field `{}`", ident.name),
                    ident.span,
                );
                continue;
            }
            if !seen.insert(ident.name.as_str()) {
                self.error(
                    "E014",
                    format!("duplicate field `{}` in {what}", ident.name),
                    ident.span,
                );
                continue;
            }
            out.push(ident.name.clone());
        }
        let _ = span;
        out
    }

    fn lower_field(&mut self, f: &ast::FieldDecl, table_names: &HashSet<&str>) -> Option<Field> {
        let ty = match &f.ty.kind {
            ast::TypeExprKind::Text => Type::Text,
            ast::TypeExprKind::Int => Type::Int,
            ast::TypeExprKind::Float => Type::Float,
            ast::TypeExprKind::Bool => Type::Bool,
            ast::TypeExprKind::Timestamp => Type::Timestamp,
            ast::TypeExprKind::Uuid => Type::Uuid,
            ast::TypeExprKind::Named(name) => {
                if !table_names.contains(name.as_str()) {
                    self.error(
                        "E003",
                        format!("unknown type `{name}` (not a builtin, not a declared table)"),
                        f.ty.span,
                    );
                    return None;
                }
                Type::Ref {
                    table: name.clone(),
                    on_delete: f
                        .on_delete
                        .as_ref()
                        .map(|c| match c.kind {
                            ast::OnDeleteKind::Restrict => OnDelete::Restrict,
                            ast::OnDeleteKind::Cascade => OnDelete::Cascade,
                            ast::OnDeleteKind::SetNull => OnDelete::SetNull,
                        })
                        .unwrap_or(OnDelete::Restrict),
                }
            }
            ast::TypeExprKind::Set(members) => self.lower_set(members, &f.name.name, f.ty.span),
        };

        let is_ref = matches!(ty, Type::Ref { .. });
        if let Some(clause) = &f.on_delete {
            if !is_ref {
                self.error(
                    "E015",
                    format!("`on delete` on `{}`, which is not a reference", f.name.name),
                    clause.span,
                );
            }
            // `set null` writes NULL into this field — the field must be
            // able to hold one. (Key fields are never optional, so a
            // key-part ref is rejected here too.)
            if clause.kind == ast::OnDeleteKind::SetNull && !f.optional {
                self.error(
                    "E040",
                    format!(
                        "`on delete set null` on `{}`, which is required; \
                         only an optional reference can be nulled",
                        f.name.name
                    ),
                    clause.span,
                );
            }
        }

        let default = match &f.default {
            None => None,
            Some(expr) => {
                if is_ref {
                    self.error(
                        "E016",
                        format!("reference field `{}` cannot have a default", f.name.name),
                        expr.span(),
                    );
                    None
                } else {
                    self.lower_default(expr, &ty, &f.name.name)
                }
            }
        };

        Some(Field {
            name: f.name.name.clone(),
            ty,
            optional: f.optional,
            unique: f.unique,
            default,
        })
    }

    /// Lower a closed-set type (RFD 0013), reporting its laws (E043): at
    /// least two members, each non-empty, all distinct. The `Type::Set` is
    /// returned even when a law fails so the default/seed checks against it
    /// do not cascade a second, confusing diagnostic.
    fn lower_set(&mut self, members: &[ast::SetMember], field: &str, span: Span) -> Type {
        if members.len() < 2 {
            self.error(
                "E043",
                format!(
                    "closed-set type of `{field}` needs at least two values \
                     (a one-value set is a constant, not a choice)"
                ),
                span,
            );
        }
        let mut seen: HashSet<&str> = HashSet::new();
        for m in members {
            if m.value.is_empty() {
                self.error(
                    "E043",
                    format!("closed-set type of `{field}` has an empty value (members must be non-empty)"),
                    m.span,
                );
            } else if !seen.insert(m.value.as_str()) {
                self.error(
                    "E043",
                    format!("closed-set type of `{field}` repeats the value `{}`", m.value),
                    m.span,
                );
            }
        }
        Type::Set {
            values: members.iter().map(|m| m.value.clone()).collect(),
        }
    }

    fn lower_default(
        &mut self,
        expr: &ast::DefaultExpr,
        ty: &Type,
        field: &str,
    ) -> Option<DefaultValue> {
        let ok = |v| Some(v);
        match (expr, ty) {
            (ast::DefaultExpr::Auto(_), Type::Uuid) => ok(DefaultValue::Auto),
            (ast::DefaultExpr::Now(_), Type::Timestamp) => ok(DefaultValue::Now),
            (ast::DefaultExpr::Lit(ast::Lit::Str(v, _)), Type::Text) => {
                ok(DefaultValue::Str { value: v.clone() })
            }
            (ast::DefaultExpr::Lit(ast::Lit::Int(v, _)), Type::Int) => {
                ok(DefaultValue::Int { value: *v })
            }
            (ast::DefaultExpr::Lit(ast::Lit::Float(v, _)), Type::Float) => {
                ok(DefaultValue::Float { value: *v })
            }
            (ast::DefaultExpr::Lit(ast::Lit::Bool(v, _)), Type::Bool) => {
                ok(DefaultValue::Bool { value: *v })
            }
            // The set IS the type: a string default must be one of its
            // members, else the default violates the field's own type (E009).
            (ast::DefaultExpr::Lit(ast::Lit::Str(v, _)), Type::Set { values }) => {
                if values.iter().any(|m| m == v) {
                    ok(DefaultValue::Str { value: v.clone() })
                } else {
                    self.error(
                        "E009",
                        format!(
                            "default `{v}` for `{field}` is not one of its values ({})",
                            values.join(" | ")
                        ),
                        expr.span(),
                    );
                    None
                }
            }
            _ => {
                self.error(
                    "E009",
                    format!(
                        "default is incompatible with the type of `{field}` \
                         (`auto` fits uuid, `now` fits timestamp, literals fit their own type)"
                    ),
                    expr.span(),
                );
                None
            }
        }
    }

    /// E010: references must target single-key tables.
    fn check_ref_targets(
        &mut self,
        tables: &[Table],
        _spans: &HashMap<String, Span>,
        file: &ast::File,
    ) {
        let composite: HashSet<&str> = tables
            .iter()
            .filter(|t| t.key.len() > 1)
            .map(|t| t.name.as_str())
            .collect();
        if composite.is_empty() {
            return;
        }
        for decl in &file.tables {
            for item in &decl.items {
                if let ast::TableItem::Field(f) = item {
                    if let ast::TypeExprKind::Named(target) = &f.ty.kind {
                        if composite.contains(target.as_str()) {
                            self.error(
                                "E010",
                                format!(
                                    "cannot reference `{target}`: its key is composite \
                                     (v0 references target single-key tables)"
                                ),
                                f.ty.span,
                            );
                        }
                    }
                }
            }
        }
    }

    /// E017: a table's key type must not resolve through references back to
    /// itself (`a.key: b` + `b.key: a`).
    fn check_key_cycles(&mut self, tables: &[Table], spans: &HashMap<String, Span>) {
        for table in tables {
            let mut visited: HashSet<&str> = HashSet::new();
            let mut current = table;
            loop {
                if !visited.insert(current.name.as_str()) {
                    self.error(
                        "E017",
                        format!(
                            "key of table `{}` resolves through a reference cycle",
                            table.name
                        ),
                        spans.get(&table.name).copied().unwrap_or(Span::new(0, 0)),
                    );
                    break;
                }
                let Some(key_field) = current.single_key() else {
                    break;
                };
                let Type::Ref { table: target, .. } = &key_field.ty else {
                    break;
                };
                let Some(next) = tables.iter().find(|t| &t.name == target) else {
                    break;
                };
                current = next;
            }
        }
    }

    // ---- seed --------------------------------------------------------

    fn check_seed(&mut self, file: &ast::File, tables: &[Table]) -> Vec<SeedRow> {
        let mut rows = Vec::new();
        // binding name -> table it holds a row of
        let mut bindings: HashMap<String, String> = HashMap::new();

        for block in &file.seeds {
            for stmt in &block.stmts {
                let Some(table) = tables.iter().find(|t| t.name == stmt.table.name) else {
                    self.error(
                        "E020",
                        format!("seed row names unknown table `{}`", stmt.table.name),
                        stmt.table.span,
                    );
                    continue;
                };

                let mut fields: BTreeMap<String, SeedValue> = BTreeMap::new();
                // Fields the author wrote, even if their value failed its own
                // check — E022 must not cascade onto a provided-but-invalid value.
                let mut provided: HashSet<String> = HashSet::new();
                for (name, value) in &stmt.fields {
                    let Some(field) = table.field(&name.name) else {
                        self.error(
                            "E021",
                            format!(
                                "seed row for `{}` names unknown field `{}`",
                                table.name, name.name
                            ),
                            name.span,
                        );
                        continue;
                    };
                    if !provided.insert(field.name.clone()) {
                        self.error(
                            "E028",
                            format!("seed row sets `{}` twice", field.name),
                            name.span,
                        );
                        continue;
                    }
                    if let Some(v) = self.check_seed_value(field, value, tables, &bindings) {
                        fields.insert(field.name.clone(), v);
                    }
                }

                // E022: required fields with no default must be present.
                for field in &table.fields {
                    if !field.optional && field.default.is_none() && !provided.contains(&field.name)
                    {
                        self.error(
                            "E022",
                            format!(
                                "seed row for `{}` omits required field `{}` (no default)",
                                table.name, field.name
                            ),
                            stmt.span,
                        );
                    }
                }

                if let Some(binding) = &stmt.binding {
                    if bindings.contains_key(&binding.name) {
                        self.error(
                            "E027",
                            format!("duplicate seed binding `{}`", binding.name),
                            binding.span,
                        );
                    } else {
                        bindings.insert(binding.name.clone(), table.name.clone());
                    }
                }

                rows.push(SeedRow {
                    table: table.name.clone(),
                    binding: stmt.binding.as_ref().map(|b| b.name.clone()),
                    fields,
                });
            }
        }
        rows
    }

    fn check_seed_value(
        &mut self,
        field: &Field,
        value: &ast::SeedValue,
        tables: &[Table],
        bindings: &HashMap<String, String>,
    ) -> Option<SeedValue> {
        match value {
            ast::SeedValue::Binding(ident) => {
                let Type::Ref { table: target, .. } = &field.ty else {
                    self.error(
                        "E026",
                        format!(
                            "field `{}` is not a reference; a seed binding cannot be its value",
                            field.name
                        ),
                        ident.span,
                    );
                    return None;
                };
                let Some(bound_table) = bindings.get(&ident.name) else {
                    self.error(
                        "E024",
                        format!(
                            "unknown seed binding `{}` (bindings must be defined earlier in the seed)",
                            ident.name
                        ),
                        ident.span,
                    );
                    return None;
                };
                if bound_table != target {
                    self.error(
                        "E025",
                        format!(
                            "binding `{}` holds a `{bound_table}` row, but `{}` references `{target}`",
                            ident.name, field.name
                        ),
                        ident.span,
                    );
                    return None;
                }
                Some(SeedValue::Ref {
                    binding: ident.name.clone(),
                })
            }
            ast::SeedValue::Lit(lit) => {
                // Literal against the field's *value* type (refs bottom out
                // at the target key's type). Format checked at compile time.
                let value_type = resolve_value_type(&field.ty, tables);
                let mismatch = |c: &mut Checker, expected: &str| {
                    c.error(
                        "E023",
                        format!("seed value for `{}` must be {expected}", field.name),
                        lit.span(),
                    );
                };
                match (lit, value_type) {
                    (ast::Lit::Str(v, _), Type::Text) => Some(SeedValue::Str(v.clone())),
                    // A closed-set value type: the literal must be a member
                    // (the same law the floor and the derived CHECK enforce).
                    (ast::Lit::Str(v, span), Type::Set { values }) => {
                        if values.iter().any(|m| m == v) {
                            Some(SeedValue::Str(v.clone()))
                        } else {
                            self.error(
                                "E023",
                                format!(
                                    "seed value `{v}` for `{}` is not one of its values ({})",
                                    field.name,
                                    values.join(" | ")
                                ),
                                *span,
                            );
                            None
                        }
                    }
                    (_, Type::Set { values }) => {
                        mismatch(self, &format!("one of {}", values.join(" | ")));
                        None
                    }
                    (ast::Lit::Str(v, span), Type::Uuid) => match uuid::Uuid::parse_str(v) {
                        Ok(u) => Some(SeedValue::Str(u.to_string())),
                        Err(_) => {
                            self.error(
                                "E023",
                                format!("seed value for `{}` is not a valid uuid", field.name),
                                *span,
                            );
                            None
                        }
                    },
                    (ast::Lit::Str(v, span), Type::Timestamp) => {
                        use time::format_description::well_known::Rfc3339;
                        match time::OffsetDateTime::parse(v, &Rfc3339) {
                            Ok(_) => Some(SeedValue::Str(v.clone())),
                            Err(_) => {
                                self.error(
                                    "E023",
                                    format!(
                                        "seed value for `{}` is not an RFC 3339 timestamp",
                                        field.name
                                    ),
                                    *span,
                                );
                                None
                            }
                        }
                    }
                    (ast::Lit::Int(v, _), Type::Int) => Some(SeedValue::Int(*v)),
                    (ast::Lit::Float(v, _), Type::Float) => Some(SeedValue::Float(*v)),
                    (ast::Lit::Bool(v, _), Type::Bool) => Some(SeedValue::Bool(*v)),
                    (_, Type::Text) => {
                        mismatch(self, "a string");
                        None
                    }
                    (_, Type::Int) => {
                        mismatch(self, "an integer");
                        None
                    }
                    (_, Type::Float) => {
                        mismatch(self, "a float (write `0.0`, not `0`)");
                        None
                    }
                    (_, Type::Bool) => {
                        mismatch(self, "a boolean");
                        None
                    }
                    (_, Type::Uuid) => {
                        mismatch(self, "a uuid string");
                        None
                    }
                    (_, Type::Timestamp) => {
                        mismatch(self, "an RFC 3339 timestamp string");
                        None
                    }
                    (_, Type::Ref { .. }) => unreachable!("value type never a ref"),
                }
            }
        }
    }
}

/// Resolve a type to its value type against a table list (checker-side
/// mirror of [`Contract::value_type`], usable before the contract exists).
fn resolve_value_type<'a>(ty: &'a Type, tables: &'a [Table]) -> &'a Type {
    match ty {
        Type::Ref { table, .. } => {
            let Some(target) = tables.iter().find(|t| &t.name == table) else {
                return ty; // unresolved elsewhere; avoid cascading
            };
            let Some(key_field) = target.single_key() else {
                return ty;
            };
            resolve_value_type(&key_field.ty, tables)
        }
        other => other,
    }
}

/// Derived errors (§6.1). Never hand-written.
fn derive_errors(table: &Table, all: &[Table]) -> Vec<DerivedError> {
    let mut errors = Vec::new();
    let t = &table.name;

    if !table.key.is_empty() {
        errors.push(DerivedError {
            code: format!("{t}_already_exists"),
            kind: ErrorKind::Key,
            fields: table.key.clone(),
            status: 409,
        });
    }
    for field in &table.fields {
        if field.unique {
            errors.push(DerivedError {
                code: format!("{t}_{}_taken", field.name),
                kind: ErrorKind::Unique,
                fields: vec![field.name.clone()],
                status: 409,
            });
        }
    }
    for group in &table.uniques {
        errors.push(DerivedError {
            code: format!("{t}_{}_taken", group.join("_")),
            kind: ErrorKind::Unique,
            fields: group.clone(),
            status: 409,
        });
    }
    for field in &table.fields {
        // Every required field that can be absent on insert (no default) or
        // cleared on update (any non-key field). Required key fields with a
        // default stay underived: the default satisfies insert and key
        // immutability protects update — the error is unreachable.
        let reachable = field.default.is_none() || !table.key.contains(&field.name);
        if !field.optional && reachable {
            errors.push(DerivedError {
                code: format!("{t}_{}_required", field.name),
                kind: ErrorKind::Required,
                fields: vec![field.name.clone()],
                status: 422,
            });
        }
    }
    for field in &table.fields {
        if matches!(field.ty, Type::Ref { .. }) {
            errors.push(DerivedError {
                code: format!("{t}_{}_not_found", field.name),
                kind: ErrorKind::RefNotFound,
                fields: vec![field.name.clone()],
                status: 422,
            });
        }
    }
    // Closed-set membership (RFD 0013): the field's value must be one of
    // the declared members. Same code the derived CHECK is named with.
    for field in &table.fields {
        if matches!(field.ty, Type::Set { .. }) {
            errors.push(DerivedError {
                code: format!("{t}_{}_invalid", field.name),
                kind: ErrorKind::Invalid,
                fields: vec![field.name.clone()],
                status: 422,
            });
        }
    }
    let restricted = all.iter().any(|other| {
        other.fields.iter().any(|f| {
            matches!(
                &f.ty,
                Type::Ref { table: target, on_delete: OnDelete::Restrict } if target == t
            )
        })
    });
    if restricted {
        errors.push(DerivedError {
            code: format!("{t}_restricted"),
            kind: ErrorKind::Restricted,
            fields: vec![],
            status: 409,
        });
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile;

    fn codes(source: &str) -> Vec<&'static str> {
        match compile(source) {
            Ok(_) => vec![],
            Err(diags) => diags.into_iter().map(|d| d.code).collect(),
        }
    }

    const USER: &str = "table user { key id: uuid = auto\n username: text unique }\n";

    #[test]
    fn accepts_the_reference_program() {
        let contract = compile(&format!(
            "{USER}\
             table post {{ key id: uuid = auto\n author: user\n caption: text? }}\n\
             table follow {{ key (follower, target)\n follower: user\n target: user }}\n\
             seed {{ maya = user {{ username: \"maya\" }}\n\
                     post {{ author: maya }} }}"
        ))
        .unwrap();
        assert_eq!(contract.tables.len(), 3);
        assert_eq!(contract.seed.len(), 2);
        let post = contract.table("post").unwrap();
        assert!(post
            .error_for(ErrorKind::RefNotFound, &["author"])
            .is_some());
        let user = contract.table("user").unwrap();
        assert!(user.error_for(ErrorKind::Unique, &["username"]).is_some());
        // user is restrict-referenced by post.author and follow.*
        assert!(user.error_for(ErrorKind::Restricted, &[]).is_some());
        // username has no default and is required
        assert!(user.error_for(ErrorKind::Required, &["username"]).is_some());
    }

    #[test]
    fn closed_sets_derive_invalid_and_check_their_values() {
        // a valid set field: derives `<t>_<f>_invalid`, TEXT storage
        let contract = compile(
            "table media { key id: uuid = auto\n\
               status: \"pending\" | \"ready\" | \"failed\" = \"pending\" }",
        )
        .unwrap();
        let media = contract.table("media").unwrap();
        let derr = media.error_for(ErrorKind::Invalid, &["status"]).unwrap();
        assert_eq!(derr.code, "media_status_invalid");
        assert_eq!(derr.status, 422);
        assert!(matches!(
            &media.field("status").unwrap().ty,
            Type::Set { values } if values == &["pending", "ready", "failed"]
        ));
    }

    #[test]
    fn closed_set_laws() {
        // E043: singleton, duplicate, and empty members
        assert_eq!(
            codes("table t { key id: uuid = auto\n s: \"only\" }"),
            ["E043"]
        );
        assert_eq!(
            codes("table t { key id: uuid = auto\n s: \"a\" | \"a\" }"),
            ["E043"]
        );
        assert_eq!(
            codes("table t { key id: uuid = auto\n s: \"a\" | \"\" }"),
            ["E043"]
        );
        // E043: a set may not be a key (keeps sets off the value_type-via-ref path)
        assert_eq!(
            codes("table t { key s: \"a\" | \"b\" }"),
            ["E043"]
        );
        // E009: a default must be a member
        assert_eq!(
            codes("table t { key id: uuid = auto\n s: \"a\" | \"b\" = \"c\" }"),
            ["E009"]
        );
        // E023: a seed literal must be a member
        assert_eq!(
            codes("table t { key id: uuid = auto\n s: \"a\" | \"b\" }\n seed { t { s: \"c\" } }"),
            ["E023"]
        );
        // L-B: a set may not be a record field (E034) or a fn param (E036)
        assert_eq!(
            codes("record r { s: \"a\" | \"b\" }"),
            ["E034"]
        );
        assert_eq!(
            codes("table t { key id: uuid = auto\n n: int }\n\
                   fn f(s: \"a\" | \"b\") -> t { unchecked sql(\"SELECT 1\") }"),
            ["E036"]
        );
    }

    #[test]
    fn required_derivation_covers_update_clearable_fields() {
        let contract = compile(
            "table user { key id: uuid = auto\n username: text unique\n joined_at: timestamp = now }",
        )
        .unwrap();
        let user = contract.table("user").unwrap();
        // required-with-default non-key field: clearable on update → derived
        assert!(user
            .error_for(ErrorKind::Required, &["joined_at"])
            .is_some());
        // required-with-default key field: unreachable → underived
        assert!(user.error_for(ErrorKind::Required, &["id"]).is_none());
    }

    #[test]
    fn e001_duplicate_table() {
        assert_eq!(
            codes("table a { key id: uuid = auto }\ntable a { key id: uuid = auto }"),
            vec!["E001"]
        );
    }

    #[test]
    fn e002_duplicate_field() {
        assert_eq!(
            codes("table a { key id: uuid = auto\n x: int\n x: text }"),
            vec!["E002"]
        );
    }

    #[test]
    fn e003_unknown_type() {
        assert_eq!(
            codes("table a { key id: uuid = auto\n who: nobody }"),
            vec!["E003"]
        );
    }

    #[test]
    fn e005_no_key() {
        assert_eq!(codes("table a { x: int }"), vec!["E005"]);
    }

    #[test]
    fn e006_two_keys() {
        assert_eq!(
            codes("table a { key id: uuid = auto\n key other: int }"),
            vec!["E006"]
        );
        assert_eq!(
            codes("table a { key id: uuid = auto\n a: int\n b: int\n key (a, b) }"),
            vec!["E006"]
        );
    }

    #[test]
    fn e007_composite_key_unknown_field() {
        assert_eq!(codes("table a { key (x, y)\n x: int }"), vec!["E007"]);
    }

    #[test]
    fn e008_optional_key() {
        assert_eq!(codes("table a { key id: uuid? = auto }"), vec!["E008"]);
        assert_eq!(
            codes("table a { key (x, y)\n x: int\n y: int? }"),
            vec!["E008"]
        );
    }

    #[test]
    fn e009_bad_default() {
        assert_eq!(codes("table a { key id: uuid = now }"), vec!["E009"]);
        assert_eq!(
            codes("table a { key id: uuid = auto\n n: int = \"x\" }"),
            vec!["E009"]
        );
        assert_eq!(
            codes("table a { key id: uuid = auto\n t: text = auto }"),
            vec!["E009"]
        );
    }

    #[test]
    fn e010_ref_to_composite_key_table() {
        assert_eq!(
            codes(
                "table pair { key (a, b)\n a: int\n b: int }\n\
                 table t { key id: uuid = auto\n p: pair }"
            ),
            vec!["E010"]
        );
    }

    #[test]
    fn e011_unique_group_unknown_field() {
        assert_eq!(
            codes("table a { key id: uuid = auto\n x: int\n unique (x, y) }"),
            vec!["E011"]
        );
    }

    #[test]
    fn e012_empty_table() {
        assert_eq!(codes("table a { }"), vec!["E012"]);
    }

    #[test]
    fn e014_duplicate_in_group() {
        assert_eq!(
            codes("table a { key id: uuid = auto\n x: int\n unique (x, x) }"),
            vec!["E014"]
        );
    }

    #[test]
    fn e015_on_delete_on_non_ref() {
        assert_eq!(
            codes("table a { key id: uuid = auto\n n: int on delete cascade }"),
            vec!["E015"]
        );
    }

    #[test]
    fn float_fields_defaults_and_seeds() {
        // literals fit their own type: 0.5 fits float, 0 does not
        let contract = compile(
            "table tag { key id: uuid = auto\n x: float = 0.5 }\n\
             seed { tag { x: 0.25 } }",
        )
        .unwrap();
        let x = contract.table("tag").unwrap().field("x").unwrap();
        assert_eq!(x.ty, Type::Float);
        assert!(matches!(x.default, Some(DefaultValue::Float { value }) if value == 0.5));
        assert_eq!(
            codes("table tag { key id: uuid = auto\n x: float = 0 }"),
            vec!["E009"]
        );
        assert_eq!(
            codes("table tag { key id: uuid = auto\n x: float }\nseed { tag { x: 1 } }"),
            vec!["E023"]
        );
    }

    #[test]
    fn e040_set_null_requires_optional() {
        // required ref: nothing to null into
        assert_eq!(
            codes(&format!(
                "{USER}table t {{ key id: uuid = auto\n u: user on delete set null }}"
            )),
            vec!["E040"]
        );
        // key-part refs are never optional, so they are rejected too
        assert_eq!(
            codes(&format!(
                "{USER}table t {{ key (u, n)\n u: user on delete set null\n n: int }}"
            )),
            vec!["E040"]
        );
        // optional ref: legal, lowers to SetNull, derives no restricted error
        let contract = compile(&format!(
            "{USER}table t {{ key id: uuid = auto\n u: user? on delete set null }}"
        ))
        .unwrap();
        let t = contract.table("t").unwrap();
        assert!(matches!(
            &t.field("u").unwrap().ty,
            Type::Ref { on_delete: OnDelete::SetNull, .. }
        ));
        let user = contract.table("user").unwrap();
        assert!(user.error_for(ErrorKind::Restricted, &[]).is_none());
    }

    #[test]
    fn e016_default_on_ref() {
        assert_eq!(
            codes(&format!(
                "{USER}table t {{ key id: uuid = auto\n u: user = auto }}"
            )),
            vec!["E016"]
        );
    }

    #[test]
    fn e017_key_ref_cycle() {
        assert_eq!(
            codes("table a { key b_ref: b }\ntable b { key a_ref: a }"),
            vec!["E017", "E017"]
        );
    }

    #[test]
    fn e020_seed_unknown_table() {
        assert_eq!(codes(&format!("{USER}seed {{ nope {{ }} }}")), vec!["E020"]);
    }

    #[test]
    fn e021_seed_unknown_field() {
        assert_eq!(
            codes(&format!(
                "{USER}seed {{ user {{ username: \"a\", nope: 1 }} }}"
            )),
            vec!["E021"]
        );
    }

    #[test]
    fn e022_seed_missing_required() {
        assert_eq!(codes(&format!("{USER}seed {{ user {{ }} }}")), vec!["E022"]);
    }

    #[test]
    fn e023_seed_type_mismatch() {
        assert_eq!(
            codes(&format!("{USER}seed {{ user {{ username: 42 }} }}")),
            vec!["E023"]
        );
        // uuid format is validated at compile time
        assert_eq!(
            codes(&format!(
                "{USER}seed {{ user {{ id: \"not-a-uuid\", username: \"a\" }} }}"
            )),
            vec!["E023"]
        );
    }

    #[test]
    fn e024_seed_unknown_binding() {
        assert_eq!(
            codes(&format!(
                "{USER}table post {{ key id: uuid = auto\n author: user }}\n\
                 seed {{ post {{ author: ghost }} }}"
            )),
            vec!["E024"]
        );
    }

    #[test]
    fn e025_seed_binding_wrong_table() {
        assert_eq!(
            codes(&format!(
                "{USER}table post {{ key id: uuid = auto\n author: user }}\n\
                 table save {{ key id: uuid = auto\n post: post }}\n\
                 seed {{ maya = user {{ username: \"maya\" }}\n\
                         save {{ post: maya }} }}"
            )),
            vec!["E025"]
        );
    }

    #[test]
    fn e026_seed_binding_on_non_ref() {
        assert_eq!(
            codes(&format!(
                "{USER}seed {{ maya = user {{ username: \"maya\" }}\n\
                        user {{ username: maya }} }}"
            )),
            vec!["E026"]
        );
    }

    #[test]
    fn e027_duplicate_binding() {
        assert_eq!(
            codes(&format!(
                "{USER}seed {{ maya = user {{ username: \"a\" }}\n\
                        maya = user {{ username: \"b\" }} }}"
            )),
            vec!["E027"]
        );
    }

    #[test]
    fn e028_seed_field_twice() {
        assert_eq!(
            codes(&format!(
                "{USER}seed {{ user {{ username: \"a\", username: \"b\" }} }}"
            )),
            vec!["E028"]
        );
    }

    #[test]
    fn join_table_with_ref_key_members_is_valid() {
        assert!(compile(&format!(
            "{USER}\
             table follow {{ key (follower, target)\n follower: user\n target: user }}"
        ))
        .is_ok());
    }

    #[test]
    fn self_reference_is_valid() {
        assert!(
            compile("table comment { key id: uuid = auto\n body: text\n reply_to: comment? }")
                .is_ok()
        );
    }

    // -- records and fns (E030–E039) ----------------------------------------

    #[test]
    fn accepts_records_and_fns() {
        let contract = compile(&format!(
            "{USER}\
             record stats {{ posts: int\n latest: timestamp? }}\n\
             fn rename_user(user: user, name: text) -> user ! user_username_taken | not_found {{ unchecked sql(\"S\") }}\n\
             fn find_user(name: text) -> user? {{ unchecked sql(\"S\") }}\n\
             fn tally() -> [stats] {{ unchecked sql(\"S\") }}"
        ))
        .unwrap();
        assert_eq!(contract.records.len(), 1);
        assert_eq!(contract.fns.len(), 3);
        let rename = contract.fn_def("rename_user").unwrap();
        assert!(matches!(&rename.params[0].ty, Type::Ref { table, .. } if table == "user"));
        assert_eq!(rename.errors, vec!["user_username_taken", "not_found"]);
        assert_eq!(rename.returns.arity, FnArity::One);
        assert_eq!(contract.fn_def("tally").unwrap().returns.arity, FnArity::Many);
        // a fn may share a table's name — separate namespaces
        assert!(compile(&format!(
            "{USER}\
             fn user(name: text) -> user? {{ unchecked sql(\"S\") }}"
        ))
        .is_ok());
    }

    #[test]
    fn e030_duplicate_record() {
        assert_eq!(
            codes("record a { x: int }\nrecord a { y: int }"),
            vec!["E030"]
        );
    }

    #[test]
    fn e031_record_collides_with_table() {
        assert_eq!(
            codes(&format!("{USER}record user {{ x: int }}")),
            vec!["E031"]
        );
    }

    #[test]
    fn e032_empty_record() {
        assert_eq!(codes("record a { }"), vec!["E032"]);
    }

    #[test]
    fn e033_table_only_syntax_in_record() {
        assert_eq!(codes("record a { key x: int }"), vec!["E033"]);
        assert_eq!(codes("record a { x: int unique }"), vec!["E033"]);
        assert_eq!(codes("record a { x: int = 3 }"), vec!["E033"]);
        assert_eq!(
            codes("record a { x: int\n y: int\n unique (x, y) }"),
            vec!["E033"]
        );
    }

    #[test]
    fn e034_non_scalar_record_field() {
        assert_eq!(
            codes(&format!("{USER}record a {{ who: user }}")),
            vec!["E034"]
        );
    }

    #[test]
    fn e035_duplicate_fn() {
        assert_eq!(
            codes(&format!(
                "{USER}\
                 fn f() -> user {{ unchecked sql(\"S\") }}\nfn f() -> user {{ unchecked sql(\"S\") }}"
            )),
            vec!["E035"]
        );
    }

    #[test]
    fn e036_bad_param_types() {
        // unknown type
        assert_eq!(
            codes(&format!("{USER}fn f(x: ghost) -> user {{ unchecked sql(\"S\") }}")),
            vec!["E036"]
        );
        // record as param
        assert_eq!(
            codes(&format!(
                "{USER}record r {{ x: int }}\nfn f(x: r) -> user {{ unchecked sql(\"S\") }}"
            )),
            vec!["E036"]
        );
        // composite-key table as param: no single key scalar to bind
        assert_eq!(
            codes(&format!(
                "{USER}\
                 table follow {{ key (follower, target)\n follower: user\n target: user }}\n\
                 fn f(x: follow) -> user {{ unchecked sql(\"S\") }}"
            )),
            vec!["E036"]
        );
    }

    #[test]
    fn e037_unknown_return_shape() {
        assert_eq!(
            codes(&format!("{USER}fn f() -> ghost {{ unchecked sql(\"S\") }}")),
            vec!["E037"]
        );
    }

    #[test]
    fn e038_duplicate_param() {
        assert_eq!(
            codes(&format!(
                "{USER}fn f(a: int, a: text) -> user {{ unchecked sql(\"S\") }}"
            )),
            vec!["E038"]
        );
    }

    #[test]
    fn e039_duplicate_error_codes() {
        assert_eq!(
            codes(&format!(
                "{USER}fn f() -> user ! not_found | not_found {{ unchecked sql(\"S\") }}"
            )),
            vec!["E039"]
        );
    }

    #[test]
    fn unknown_error_codes_mint_refusals() {
        // RFD 0012 §2.3: a code in the vocabulary is a reference; a code
        // in neither population is a refusal minted by this fn
        let contract = compile(&format!(
            "{USER}fn f(name: text) -> user ! account_private | user_username_taken | not_found {{ unchecked sql(\"SELECT * FROM user WHERE username = :name\") }}"
        ))
        .expect("mints, not errors");
        let f = &contract.fns[0];
        assert_eq!(
            f.errors,
            vec!["account_private", "user_username_taken", "not_found"]
        );
        // only the unbacked code is minted — references are not refusals
        assert_eq!(f.refusals, vec!["account_private"]);
    }
}

//! Provider generation spike (uhura#29): .wire — a projection/contract language
//! over the Spock schema. Parses a .wire declaration, validates it against the
//! compiler-emitted contract, and generates the provider artifacts: contract
//! types, view types, snapshot query, dispatch, refusals, assets, and the module.

use pest::Parser;
use pest_derive::Parser;
use serde_json::Value;

#[derive(Parser)]
#[grammar = "provider_gen/wire.pest"]
struct WireParser;

#[derive(Debug, PartialEq)]
pub struct Field {
    pub name: String,
    /// Original `.wire` type token (e.g. `post.id`) — used for schema validation.
    pub source: String,
    pub ty: String,
}

#[derive(Debug, PartialEq)]
pub struct MutationDecl {
    pub name: String,
    /// Backend operation kind override (`op choose_image_request`); defaults to snake_case(name)
    pub op: Option<String>,
    pub fields: Vec<Field>,
    pub policy: String,
}

#[derive(Debug, PartialEq)]
pub struct CallSpec {
    pub fn_name: String,
    /// For `if <flag>` branches: (flag field, the value this call handles)
    pub when: Option<(String, bool)>,
    /// Call argument names — extracted from the operation object to form the RPC body
    pub args: Vec<String>,
    /// Route key in the refusal whitelist (`route feed/like-post`) — explicit, never derived
    pub route: Option<String>,
    pub allows: Vec<String>,
}

#[derive(Debug, PartialEq, Default)]
pub struct SettlementDecl {
    pub extras: Vec<(String, Vec<Field>)>,
}

#[derive(Debug, PartialEq)]
pub enum ViewEntry {
    /// Bare source column, with an optional projection (`author -> User`).
    Column {
        name: String,
        projected: Option<String>,
    },
    /// `x = ago(col)` → Text
    Ago { name: String, column: String },
    /// `x = count <table> where …` → Nat
    Count { name: String, table: String },
    /// `x = exists <table> where …` → Bool
    Exists { name: String, table: String },
    /// `x = match <col> { … }` → PascalCase(x) sum type (variant bodies from the arms)
    Match {
        name: String,
        column: String,
        arms: Vec<MatchArm>,
    },
    /// `x = tiles of <table> …` → Seq<Tile>
    Tiles { name: String, table: String },
    /// `x = row as T` → T (projection of the row itself)
    RowAs { name: String, ty: String },
}

#[derive(Debug, PartialEq)]
pub struct MatchArm {
    pub tag: String,
    pub variant: String,
    pub fields: Vec<(String, VariantField)>,
}

#[derive(Debug, PartialEq)]
pub enum VariantField {
    /// `<col> as <class>` — project a column of the view's source table as an asset class
    Scalar { column: String, class: String },
    /// `each <table>.<col> as <class> …` — sequence projection over child rows
    Each {
        table: String,
        column: String,
        class: String,
    },
}

#[derive(Debug, PartialEq)]
pub struct ViewDecl {
    pub name: String,
    pub source: String,
    pub entries: Vec<ViewEntry>,
}

#[derive(Debug, PartialEq)]
pub struct SnapshotRead {
    pub table: String,
    /// `read carousel_slide as slides` — explicit override for irregular aliases.
    pub alias: Option<String>,
}

#[derive(Debug, Default)]
pub struct WireFile {
    pub app: Option<String>,
    /// Explicit video mappings from the fixtures block (no manifest source to derive from)
    pub videos: Vec<(String, String)>,
    pub snapshot_cap: Option<u32>,
    pub snapshot_reads: Vec<SnapshotRead>,
    pub views: Vec<ViewDecl>,
    pub mutations: Vec<MutationDecl>,
    pub settlement: SettlementDecl,
}

#[derive(Debug)]
pub struct ParseError(pub String);

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
impl std::error::Error for ParseError {}

/// `.wire` field type → machine-side type name.
/// `<table>.id` projects to `<Table>Id`; scalars use the 0.4 prelude names.
fn machine_type(ty: &str) -> Result<String, ParseError> {
    match ty {
        "text" => Ok("Text".to_string()),
        "bool" => Ok("Bool".to_string()),
        "int" => Ok("Int".to_string()),
        "nat" => Ok("Nat".to_string()),
        other => match other.split_once('.') {
            Some((table, "id")) => {
                let mut chars = table.chars();
                let head = chars
                    .next()
                    .ok_or_else(|| ParseError("empty table name".into()))?;
                Ok(format!("{}{}Id", head.to_ascii_uppercase(), chars.as_str()))
            }
            _ => Err(ParseError(format!("unknown field type `{other}`"))),
        },
    }
}

fn collect_fields(pair: pest::iterators::Pair<Rule>) -> Result<Vec<Field>, ParseError> {
    let mut out = Vec::new();
    for field in pair.into_inner() {
        let mut inner = field.into_inner();
        let name = inner.next().expect("field name").as_str().to_string();
        let source = inner.next().expect("field type").as_str().to_string();
        let ty = machine_type(&source)?;
        out.push(Field { name, source, ty });
    }
    Ok(out)
}

fn parse_arm(pair: pest::iterators::Pair<Rule>) -> Result<MatchArm, ParseError> {
    let mut inner = pair.into_inner();
    let tag = inner
        .next()
        .expect("arm tag")
        .as_str()
        .trim_matches('"')
        .to_string();
    let variant = inner.next().expect("arm variant").as_str().to_string();
    let mut fields = Vec::new();
    for vfield in inner {
        let mut parts = vfield.into_inner();
        let name = parts
            .next()
            .expect("variant field name")
            .as_str()
            .to_string();
        let expr = parts
            .next()
            .expect("variant field expr")
            .into_inner()
            .next()
            .expect("vfexpr variant");
        let rule = expr.as_rule();
        let mut ops = expr.into_inner();
        let field = match rule {
            Rule::asof_e => VariantField::Scalar {
                column: ops.next().expect("column").as_str().to_string(),
                class: ops.next().expect("class").as_str().to_string(),
            },
            Rule::each_e => VariantField::Each {
                table: ops.next().expect("table").as_str().to_string(),
                column: ops.next().expect("column").as_str().to_string(),
                class: ops.next().expect("class").as_str().to_string(),
            },
            other => return Err(ParseError(format!("unexpected variant field {other:?}"))),
        };
        fields.push((name, field));
    }
    Ok(MatchArm {
        tag,
        variant,
        fields,
    })
}

/// Asset class → machine type. Only storage_object columns can be asset-projected.
fn asset_type(class: &str) -> Option<&'static str> {
    match class {
        "image" => Some("ImageRef"),
        "url" => Some("Text"),
        _ => None,
    }
}

pub fn parse(source: &str) -> Result<WireFile, ParseError> {
    let mut pairs = WireParser::parse(Rule::file, source).map_err(|e| ParseError(e.to_string()))?;
    let file = pairs.next().expect("file rule");
    let mut out = WireFile::default();

    for item in file.into_inner() {
        match item.as_rule() {
            Rule::snapshot => {
                for entry in item.into_inner() {
                    match entry.as_rule() {
                        Rule::cap_decl => {
                            let n = entry.into_inner().next().expect("cap number");
                            out.snapshot_cap = n.as_str().parse().ok();
                        }
                        Rule::read_decl => {
                            for read in entry.into_inner() {
                                let mut parts = read.into_inner();
                                let table = parts.next().expect("read table").as_str().to_string();
                                let alias = parts.next().map(|p| p.as_str().to_string());
                                out.snapshot_reads.push(SnapshotRead { table, alias });
                            }
                        }
                        _ => {}
                    }
                }
            }
            Rule::view => {
                let mut inner = item.into_inner();
                let name = inner.next().expect("view name").as_str().to_string();
                let source = inner.next().expect("view source").as_str().to_string();
                let mut entries = Vec::new();
                for entry in inner {
                    match entry.as_rule() {
                        Rule::column => {
                            let mut parts = entry.into_inner();
                            let name = parts.next().expect("column name").as_str().to_string();
                            let projected = parts.next().map(|p| p.as_str().to_string());
                            entries.push(ViewEntry::Column { name, projected });
                        }
                        Rule::computed => {
                            let mut parts = entry.into_inner();
                            let name = parts.next().expect("computed name").as_str().to_string();
                            let expr = parts
                                .next()
                                .expect("computed expr")
                                .into_inner()
                                .next()
                                .expect("vexpr variant");
                            let rule = expr.as_rule();
                            if rule == Rule::match_e {
                                let mut inner = expr.into_inner();
                                let column =
                                    inner.next().expect("match column").as_str().to_string();
                                let arms = inner.map(parse_arm).collect::<Result<_, _>>()?;
                                entries.push(ViewEntry::Match { name, column, arms });
                                continue;
                            }
                            let first = expr
                                .into_inner()
                                .next()
                                .expect("expr operand")
                                .as_str()
                                .to_string();
                            entries.push(match rule {
                                Rule::ago_e => ViewEntry::Ago {
                                    name,
                                    column: first,
                                },
                                Rule::count_e => ViewEntry::Count { name, table: first },
                                Rule::exists_e => ViewEntry::Exists { name, table: first },
                                Rule::tiles_e => ViewEntry::Tiles { name, table: first },
                                Rule::rowas_e => ViewEntry::RowAs { name, ty: first },
                                other => {
                                    return Err(ParseError(format!(
                                        "unexpected view expression {other:?}"
                                    )))
                                }
                            });
                        }
                        _ => {}
                    }
                }
                out.views.push(ViewDecl {
                    name,
                    source,
                    entries,
                });
            }
            Rule::fixtures => {
                for entry in item.into_inner() {
                    if entry.as_rule() == Rule::video_decl {
                        let mut parts = entry.into_inner();
                        let name = parts
                            .next()
                            .expect("video name")
                            .as_str()
                            .trim_matches('"')
                            .to_string();
                        let file = parts
                            .next()
                            .expect("video file")
                            .as_str()
                            .trim_matches('"')
                            .to_string();
                        out.videos.push((name, file));
                    }
                }
            }
            Rule::app_decl => {
                let s = item.into_inner().next().expect("app name");
                out.app = Some(s.as_str().trim_matches('"').to_string());
            }
            Rule::mutation => {
                let mut inner = item.into_inner();
                let name = inner.next().expect("mutation name").as_str().to_string();
                let mut op = None;
                let mut fields = Vec::new();
                let mut policy = String::new();
                for part in inner {
                    match part.as_rule() {
                        Rule::ident => op = Some(part.as_str().to_string()),
                        Rule::fields => fields = collect_fields(part)?,
                        Rule::policy => {
                            policy = part.as_str().trim_start_matches("->").trim().to_string()
                        }
                        _ => {}
                    }
                }
                out.mutations.push(MutationDecl {
                    name,
                    op,
                    fields,
                    policy,
                });
            }
            Rule::settlement => {
                for entry in item.into_inner() {
                    if entry.as_rule() == Rule::extra_decl {
                        let mut inner = entry.into_inner();
                        let name = inner.next().expect("extra name").as_str().to_string();
                        let fields = collect_fields(inner.next().expect("extra fields"))?;
                        out.settlement.extras.push((name, fields));
                    }
                }
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Extract `call <fn>(...) [route g/name] allow e1, e2` clauses from a policy string.
/// `local {...}` / `host {...}` policies have no calls, so the list is empty.
pub fn policy_calls(policy: &str) -> Vec<CallSpec> {
    let mut out = Vec::new();
    // `if <flag> call A(...) ... else call B(...)` — first call handles true, second false
    let flag = policy.trim_start().strip_prefix("if ").map(|rest| {
        let end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        rest[..end].to_string()
    });
    let mut call_index = 0usize;
    let mut rest = policy;
    while let Some(pos) = rest.find("call ") {
        // "call" must not be part of a longer identifier
        if pos > 0
            && rest[..pos]
                .chars()
                .next_back()
                .is_some_and(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            rest = &rest[pos + 5..];
            continue;
        }
        rest = &rest[pos + 5..];
        let name_end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        let fn_name = rest[..name_end].to_string();
        rest = &rest[name_end..];
        let mut args = Vec::new();
        if let Some(open) = rest.find('(') {
            if let Some(close) = rest.find(')') {
                args = rest[open + 1..close]
                    .split(',')
                    .map(|a| a.trim().to_string())
                    .filter(|a| !a.is_empty())
                    .collect();
                rest = &rest[close + 1..];
            }
        }
        let mut route = None;
        let trimmed = rest.trim_start();
        if let Some(after) = trimmed.strip_prefix("route ") {
            let token_end = after.find(char::is_whitespace).unwrap_or(after.len());
            route = Some(after[..token_end].to_string());
            rest = &after[token_end..];
        }
        let mut allows = Vec::new();
        let trimmed = rest.trim_start();
        if let Some(list) = trimmed.strip_prefix("allow ") {
            let mut consumed = rest.len() - trimmed.len() + 6;
            for word in list.split_whitespace() {
                if word == "else" {
                    break;
                }
                let had_comma = word.ends_with(',');
                let ident = word.trim_end_matches(',');
                consumed += word.len() + 1;
                if !ident.is_empty() {
                    allows.push(ident.to_string());
                }
                if !had_comma {
                    break;
                }
            }
            rest = &rest[consumed.min(rest.len())..];
        }
        if !fn_name.is_empty() {
            let when = flag.as_ref().map(|f| (f.clone(), call_index == 0));
            call_index += 1;
            out.push(CallSpec {
                fn_name,
                when,
                args,
                route,
                allows,
            });
        }
    }
    out
}

/// Mutation → backend routing table (JSON). The runtime decides branches, RPC
/// arguments, and refusal routes from this table alone — it owns no logic.
pub fn generate_routing(file: &WireFile) -> String {
    let mut out = String::from("{\n");
    for (i, m) in file.mutations.iter().enumerate() {
        let kind = m.op.clone().unwrap_or_else(|| snake(&m.name));
        let policy = m.policy.trim_start();
        let mode = if policy.starts_with("local") {
            "local"
        } else if policy.starts_with("host") {
            "host"
        } else {
            "call"
        };
        out.push_str(&format!(
            "  \"{}\": {{ \"kind\": \"{kind}\", \"mode\": \"{mode}\"",
            m.name
        ));
        let calls = policy_calls(&m.policy);
        if let Some((flag, _)) = calls.first().and_then(|c| c.when.clone()) {
            out.push_str(&format!(", \"flag\": \"{flag}\""));
        }
        if !calls.is_empty() {
            out.push_str(", \"calls\": [");
            for (j, c) in calls.iter().enumerate() {
                if j > 0 {
                    out.push_str(", ");
                }
                let when = match &c.when {
                    Some((_, v)) => v.to_string(),
                    None => "null".to_string(),
                };
                let args = c
                    .args
                    .iter()
                    .map(|a| format!("\"{a}\""))
                    .collect::<Vec<_>>()
                    .join(", ");
                out.push_str(&format!(
                    "{{ \"when\": {when}, \"fn\": \"{}\", \"route\": {}, \"args\": [{args}] }}",
                    c.fn_name,
                    match &c.route {
                        Some(r) => format!("\"{r}\""),
                        None => "null".to_string(),
                    }
                ));
            }
            out.push(']');
        }
        out.push_str(" }");
        if i + 1 < file.mutations.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push('}');
    out
}

/// Schema validation: every table/fn/error a .wire file references must exist
/// in the contract. Violations come back as a readable list (no silent passes).
pub fn validate_against(file: &WireFile, schema: &SpockSchema) -> Vec<String> {
    let mut problems = Vec::new();
    for read in &file.snapshot_reads {
        if !schema.has_table(&read.table) {
            problems.push(format!("snapshot reads unknown table `{}`", read.table));
        }
    }
    for m in &file.mutations {
        for f in &m.fields {
            if let Some((table, "id")) = f.source.split_once('.') {
                if !schema.has_table(table) {
                    problems.push(format!(
                        "{}.{} references unknown table `{table}`",
                        m.name, f.name
                    ));
                }
            }
        }
        for call in policy_calls(&m.policy) {
            let fn_name = &call.fn_name;
            match schema.find_fn(fn_name) {
                None => problems.push(format!("{} calls unknown fn `{fn_name}`", m.name)),
                Some(spock_fn) => {
                    for allowed in &call.allows {
                        if !spock_fn.errors.iter().any(|e| e == allowed) {
                            problems.push(format!(
                                "{} allows `{allowed}` but `{fn_name}` declares only {:?}",
                                m.name, spock_fn.errors
                            ));
                        }
                    }
                }
            }
            if !call.allows.is_empty() && call.route.is_none() {
                problems.push(format!(
                    "{} call `{fn_name}` has an allow list but no `route`",
                    m.name
                ));
            }
        }
    }
    problems
}

/// Scalar column type → machine type. FK/enum columns never reach here.
fn column_machine_type(table: &str, column: &SpockColumn) -> Option<String> {
    if column.key && column.base == "uuid" {
        let mut chars = table.chars();
        let head = chars.next()?;
        return Some(format!("{}{}Id", head.to_ascii_uppercase(), chars.as_str()));
    }
    match column.base.as_str() {
        "text" => Some("Text".to_string()),
        "bool" => Some("Bool".to_string()),
        "int" => Some("Int".to_string()),
        _ => None,
    }
}

fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn kebab(name: &str) -> String {
    name.replace('_', "-")
}

fn screaming(name: &str) -> String {
    name.to_ascii_uppercase()
}

/// Mutation surface → the adapter's `toBackendOperation` switch body (TS text).
/// kind = `op` override or snake_case(name); field mapping is derived from types.
pub fn generate_dispatch(file: &WireFile) -> Result<String, Vec<String>> {
    let mut problems = Vec::new();
    let Some(app) = &file.app else {
        return Err(vec!["missing `app \"Name\";` declaration".to_string()]);
    };
    let mut out = String::new();
    for m in &file.mutations {
        let kind = m.op.clone().unwrap_or_else(|| snake(&m.name));
        out.push_str(&format!("    case \"{}\":\n", m.name));
        if m.fields.is_empty() {
            out.push_str(&format!(
                "      return {{ request, operation: {{ kind: \"{kind}\" }} }};\n"
            ));
            continue;
        }
        out.push_str("      return {\n        request,\n        operation: {\n");
        out.push_str(&format!("          kind: \"{kind}\",\n"));
        for f in &m.fields {
            let conv = match f.source.split_once('.') {
                Some((table, "id")) => format!(
                    "keyText(requiredField(fields, \"{}\"), {}_ID_TYPE)",
                    f.name,
                    screaming(table)
                ),
                _ => match f.source.as_str() {
                    "bool" => format!("boolValue(requiredField(fields, \"{}\"))", f.name),
                    "text" => format!("textValue(requiredField(fields, \"{}\"))", f.name),
                    other => {
                        problems.push(format!(
                            "{}.{}: no dispatch conversion for `{other}`",
                            m.name, f.name
                        ));
                        continue;
                    }
                },
            };
            out.push_str(&format!("          {}: {conv},\n", f.name));
        }
        out.push_str("        },\n      };\n");
    }
    out.push_str(&format!(
        "    default:\n      throw new TypeError(`unsupported {app} mutation \\`${{mutation}}\\``);\n"
    ));
    if problems.is_empty() {
        Ok(out)
    } else {
        Err(problems)
    }
}

/// Route key → kebab-case refusal list. Duplicate routes are an error.
pub fn generate_refusals(file: &WireFile) -> Result<Vec<(String, Vec<String>)>, Vec<String>> {
    let mut problems = Vec::new();
    let mut out: Vec<(String, Vec<String>)> = Vec::new();
    for m in &file.mutations {
        for call in policy_calls(&m.policy) {
            let Some(route) = call.route else { continue };
            if out.iter().any(|(r, _)| r == &route) {
                problems.push(format!("duplicate route `{route}`"));
                continue;
            }
            out.push((route, call.allows.iter().map(|a| kebab(a)).collect()));
        }
    }
    if problems.is_empty() {
        Ok(out)
    } else {
        Err(problems)
    }
}

/// Assemble the generated artifacts into a runnable provider tables module (ESM JS).
/// Logic (queue/settlement) belongs to the shared runtime; this module carries data and pure dispatch only.
pub fn generate_provider_module(
    file: &WireFile,
    schema: &SpockSchema,
) -> Result<String, Vec<String>> {
    let snapshot = generate_snapshot_query(file, schema)?;
    let dispatch = generate_dispatch(file)?;
    let refusals = generate_refusals(file)?;

    let mut id_types: Vec<String> = Vec::new();
    for m in &file.mutations {
        for f in &m.fields {
            if let Some((table, "id")) = f.source.split_once('.') {
                let name = format!("{}_ID_TYPE", screaming(table));
                if !id_types.contains(&name) {
                    id_types.push(name);
                }
            }
        }
    }

    let routing = generate_routing(file);
    let mut out = String::from("// Generated by `spock gen provider` — do not edit.\n\n");
    out.push_str(&format!("export const SNAPSHOT_QUERY = `{snapshot}`;\n\n"));
    out.push_str(&format!("export const MUTATION_ROUTING = {routing};\n\n"));
    out.push_str("export const COMMAND_REFUSALS = {\n");
    for (route, allows) in &refusals {
        let list = allows
            .iter()
            .map(|a| format!("\"{a}\""))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("  \"{route}\": [{list}],\n"));
    }
    out.push_str("};\n\n");
    let mut helper_names = vec![
        "keyText".to_string(),
        "requiredField".to_string(),
        "boolValue".to_string(),
        "textValue".to_string(),
    ];
    helper_names.extend(id_types);
    out.push_str(&format!(
        "export function toBackendOperation(mutation, request, fields, helpers) {{\n  const {{ {} }} = helpers;\n  switch (mutation) {{\n{dispatch}  }}\n}}\n",
        helper_names.join(", ")
    ));
    Ok(out)
}

/// Extract `[assets.<name>]` + `file = "..."` pairs from the asset manifest
/// (manifest.toml). alt/size/sha256 belong to the gen-assets tool and are not read here.
pub fn parse_manifest(source: &str) -> Result<Vec<(String, String)>, Vec<String>> {
    let mut problems = Vec::new();
    let mut out: Vec<(String, String)> = Vec::new();
    let mut current: Option<String> = None;
    for line in source.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("[assets.") {
            if let Some(prev) = current.take() {
                problems.push(format!("asset `{prev}` has no `file` entry"));
            }
            let name = rest.trim_end_matches(']').to_string();
            if out.iter().any(|(n, _)| n == &name) {
                problems.push(format!("duplicate asset `{name}`"));
                continue;
            }
            current = Some(name);
        } else if let Some(rest) = line.strip_prefix("file = ") {
            if let Some(name) = current.take() {
                out.push((name, rest.trim_matches('"').to_string()));
            }
        }
    }
    if let Some(prev) = current {
        problems.push(format!("asset `{prev}` has no `file` entry"));
    }
    if problems.is_empty() {
        Ok(out)
    } else {
        Err(problems)
    }
}

/// Manifest entries + explicit .wire video mappings → Play logical-asset table (TS text).
/// Name collisions are an error.
pub fn generate_play_assets(
    entries: &[(String, String)],
    videos: &[(String, String)],
) -> Result<String, Vec<String>> {
    let mut problems = Vec::new();
    for (name, _) in videos {
        if entries.iter().any(|(n, _)| n == name) {
            problems.push(format!("video `{name}` collides with a manifest asset"));
        }
    }
    if !problems.is_empty() {
        return Err(problems);
    }
    let mut out = String::from("const LOCAL_PLAY_ASSETS: Readonly<Record<string, string>> = {\n");
    for (name, file) in entries.iter().chain(videos) {
        out.push_str(&format!("  \"{name}\": \"{file}\",\n"));
    }
    out.push_str("};");
    Ok(out)
}

fn camel(name: &str) -> String {
    let mut parts = name.split('_').filter(|s| !s.is_empty());
    let mut out = parts.next().unwrap_or_default().to_string();
    for part in parts {
        let mut chars = part.chars();
        if let Some(head) = chars.next() {
            out.push(head.to_ascii_uppercase());
            out.push_str(chars.as_str());
        }
    }
    out
}

fn pluralize(name: &str) -> String {
    match name.strip_suffix('y') {
        Some(stem) => format!("{stem}ies"),
        None => format!("{name}s"),
    }
}

/// Default alias for a snapshot entry: camelCase plural (user→users, story→stories,
/// story_view→storyViews). Aliases outside this rule must be declared with `as`.
pub fn default_alias(table: &str) -> String {
    pluralize(&camel(table))
}

/// .wire snapshot declaration + schema columns → the adapter's GraphQL snapshot document.
/// FK and storage_object columns project as `name { id }`; the rest stay bare.
pub fn generate_snapshot_query(
    file: &WireFile,
    schema: &SpockSchema,
) -> Result<String, Vec<String>> {
    let mut problems = Vec::new();
    let Some(cap) = file.snapshot_cap else {
        return Err(vec![
            "snapshot has no `cap N per table` declaration".to_string()
        ]);
    };
    let mut out = String::from("\n  query UhuraSnapshot {\n");
    for read in &file.snapshot_reads {
        let Some(table) = schema.find_table(&read.table) else {
            problems.push(format!("snapshot reads unknown table `{}`", read.table));
            continue;
        };
        let alias = read
            .alias
            .clone()
            .unwrap_or_else(|| default_alias(&read.table));
        out.push_str(&format!("    {alias}: {}(limit: {cap}) {{\n", read.table));
        for column in &table.columns {
            let is_ref = column.base == "storage_object" || schema.has_table(&column.base);
            if is_ref {
                out.push_str(&format!("      {} {{ id }}\n", column.name));
            } else {
                out.push_str(&format!("      {}\n", column.name));
            }
        }
        out.push_str("    }\n");
    }
    out.push_str("  }\n");
    if problems.is_empty() {
        Ok(out)
    } else {
        Err(problems)
    }
}

fn pascal(name: &str) -> String {
    name.split('_')
        .filter(|s| !s.is_empty())
        .map(|s| {
            let mut chars = s.chars();
            match chars.next() {
                Some(head) => format!("{}{}", head.to_ascii_uppercase(), chars.as_str()),
                None => String::new(),
            }
        })
        .collect()
}

/// View declaration → machine-side record types. If type derivation disagrees with
/// the schema, return a problem list instead of generating (no speculative output).
pub fn generate_view_types(file: &WireFile, schema: &SpockSchema) -> Result<String, Vec<String>> {
    let mut problems = Vec::new();
    let mut out = String::new();
    for (i, view) in file.views.iter().enumerate() {
        let Some(table) = schema.find_table(&view.source) else {
            problems.push(format!(
                "view {} is from unknown table `{}`",
                view.name, view.source
            ));
            continue;
        };
        let _ = i;
        let mut fields = String::new();
        let mut enums: Vec<String> = Vec::new();
        for entry in &view.entries {
            let (name, ty) = match entry {
                ViewEntry::Column { name, projected } => match table.find_column(name) {
                    None => {
                        problems.push(format!(
                            "view {}: `{}` is not a column of `{}`",
                            view.name, name, view.source
                        ));
                        continue;
                    }
                    Some(column) => match projected {
                        Some(target) => {
                            if !schema.has_table(&column.base) {
                                problems.push(format!(
                                    "view {}: `{}` projects `->` but is not a foreign key",
                                    view.name, name
                                ));
                                continue;
                            }
                            (name.clone(), target.clone())
                        }
                        None => match column_machine_type(&view.source, column) {
                            Some(ty) => (name.clone(), ty),
                            None => {
                                problems.push(format!(
                                    "view {}: column `{}` of type `{}` needs an explicit projection",
                                    view.name, name, column.base
                                ));
                                continue;
                            }
                        },
                    },
                },
                ViewEntry::Ago { name, column } => {
                    match table.find_column(column) {
                        Some(c) if c.base == "timestamp" => {}
                        Some(c) => problems.push(format!(
                            "view {}: ago(`{}`) needs a timestamp, found `{}`",
                            view.name, column, c.base
                        )),
                        None => problems.push(format!(
                            "view {}: ago references unknown column `{}`",
                            view.name, column
                        )),
                    }
                    (name.clone(), "Text".to_string())
                }
                ViewEntry::Count { name, table: t } => {
                    if !schema.has_table(t) {
                        problems.push(format!(
                            "view {}: count over unknown table `{t}`",
                            view.name
                        ));
                    }
                    (name.clone(), "Nat".to_string())
                }
                ViewEntry::Exists { name, table: t } => {
                    if !schema.has_table(t) {
                        problems.push(format!(
                            "view {}: exists over unknown table `{t}`",
                            view.name
                        ));
                    }
                    (name.clone(), "Bool".to_string())
                }
                ViewEntry::Match { name, column, arms } => {
                    match table.find_column(column) {
                        None => problems.push(format!(
                            "view {}: match on unknown column `{}`",
                            view.name, column
                        )),
                        Some(c) if c.base != "enum" => problems.push(format!(
                            "view {}: match on `{}` needs an inline-enum column, found `{}`",
                            view.name, column, c.base
                        )),
                        Some(_) => {}
                    }
                    let enum_name = pascal(name);
                    let mut body = format!("pub enum {enum_name} {{\n");
                    for arm in arms {
                        body.push_str(&format!("  {} {{\n", arm.variant));
                        for (field_name, vf) in &arm.fields {
                            let ty = match vf {
                                VariantField::Scalar { column, class } => {
                                    match table.find_column(column) {
                                        Some(c) if c.base == "storage_object" => {}
                                        Some(c) => problems.push(format!(
                                            "view {}: `{}` as {class} needs storage_object, found `{}`",
                                            view.name, column, c.base
                                        )),
                                        None => problems.push(format!(
                                            "view {}: arm `{}` references unknown column `{}`",
                                            view.name, arm.tag, column
                                        )),
                                    }
                                    match asset_type(class) {
                                        Some(ty) => ty.to_string(),
                                        None => {
                                            problems.push(format!(
                                                "view {}: unknown asset class `{class}`",
                                                view.name
                                            ));
                                            continue;
                                        }
                                    }
                                }
                                VariantField::Each {
                                    table: t,
                                    column,
                                    class,
                                } => {
                                    match schema.find_table(t) {
                                        None => problems.push(format!(
                                            "view {}: each over unknown table `{t}`",
                                            view.name
                                        )),
                                        Some(sub) => match sub.find_column(column) {
                                            Some(c) if c.base == "storage_object" => {}
                                            Some(c) => problems.push(format!(
                                                "view {}: each `{t}.{column}` as {class} needs storage_object, found `{}`",
                                                view.name, c.base
                                            )),
                                            None => problems.push(format!(
                                                "view {}: `{t}` has no column `{column}`",
                                                view.name
                                            )),
                                        },
                                    }
                                    match asset_type(class) {
                                        Some(ty) => format!("Seq<{ty}>"),
                                        None => {
                                            problems.push(format!(
                                                "view {}: unknown asset class `{class}`",
                                                view.name
                                            ));
                                            continue;
                                        }
                                    }
                                }
                            };
                            body.push_str(&format!("    {field_name}: {ty},\n"));
                        }
                        body.push_str("  },\n");
                    }
                    body.push_str("}\n");
                    enums.push(body);
                    (name.clone(), enum_name)
                }
                ViewEntry::Tiles { name, table: t } => {
                    if !schema.has_table(t) {
                        problems.push(format!("view {}: tiles of unknown table `{t}`", view.name));
                    }
                    (name.clone(), "Seq<Tile>".to_string())
                }
                ViewEntry::RowAs { name, ty } => (name.clone(), ty.clone()),
            };
            fields.push_str(&format!("  {name}: {ty},\n"));
        }
        for body in enums {
            if !out.is_empty() {
                out.push('\n');
            }
            out.push_str(&body);
        }
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("pub struct {} {{\n{fields}}}\n", view.name));
    }
    if problems.is_empty() {
        Ok(out)
    } else {
        Err(problems)
    }
}

fn emit_variant(out: &mut String, name: &str, fields: &[Field]) {
    if fields.is_empty() {
        out.push_str(&format!("  {name},\n"));
    } else {
        out.push_str(&format!("  {name} {{\n"));
        for f in fields {
            out.push_str(&format!("    {}: {},\n", f.name, f.ty));
        }
        out.push_str("  },\n");
    }
}

/// Generate the machine-side contract declarations. Settlement's Accepted/Refused are
/// fixed by the 0.4 result vocabulary, so they are enforced; only extras follow the file.
pub fn generate_machine_types(file: &WireFile) -> String {
    let mut out = String::from("pub enum Mutation {\n");
    for m in &file.mutations {
        emit_variant(&mut out, &m.name, &m.fields);
    }
    out.push_str("}\n\npub enum Settlement {\n  Accepted,\n  Refused {\n    reason: Text,\n  },\n");
    for (name, fields) in &file.settlement.extras {
        emit_variant(&mut out, name, fields);
    }
    out.push_str("}\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_table_id_refs_to_machine_id_types() {
        assert_eq!(machine_type("post.id").unwrap(), "PostId");
        assert_eq!(machine_type("user.id").unwrap(), "UserId");
        assert_eq!(machine_type("story.id").unwrap(), "StoryId");
        assert_eq!(machine_type("text").unwrap(), "Text");
        assert_eq!(machine_type("bool").unwrap(), "Bool");
    }

    #[test]
    fn rejects_unknown_types_instead_of_guessing() {
        assert!(machine_type("post.author").is_err());
        assert!(machine_type("json").is_err());
    }

    #[test]
    fn unit_mutation_needs_no_fields() {
        let file = parse("mutation LoadMore -> local { resnapshot; };").unwrap();
        assert_eq!(file.mutations.len(), 1);
        assert!(file.mutations[0].fields.is_empty());
        assert_eq!(file.mutations[0].policy, "local { resnapshot; }");
    }

    #[test]
    fn malformed_source_is_a_parse_error() {
        assert!(parse("mutation { post: post.id }").is_err());
    }
}
// Schema input: consume the compiler-emitted contract JSON (`spock build` /
// GET /~contract). We never parse .spock source text — the contract is the
// truth (text parsing missed the storage_object system table) and, being
// additively frozen, a stable input.

#[derive(Debug, Default, PartialEq)]
pub struct SpockSchema {
    pub tables: Vec<SpockTable>,
    pub errors: Vec<String>,
    pub fns: Vec<SpockFn>,
}

#[derive(Debug, Default, PartialEq)]
pub struct SpockTable {
    pub name: String,
    pub columns: Vec<SpockColumn>,
}

#[derive(Debug, PartialEq)]
pub struct SpockColumn {
    pub name: String,
    /// Base type token: uuid/text/timestamp/bool/int, the target table name for
    /// FKs (including storage_object), or "enum" for inline enumerations (set).
    pub base: String,
    pub key: bool,
}

#[derive(Debug, PartialEq)]
pub struct SpockFn {
    pub name: String,
    pub errors: Vec<String>,
    pub mutating: bool,
}

impl SpockSchema {
    pub fn has_table(&self, name: &str) -> bool {
        self.tables.iter().any(|t| t.name == name)
    }
    pub fn find_table(&self, name: &str) -> Option<&SpockTable> {
        self.tables.iter().find(|t| t.name == name)
    }
    pub fn find_fn(&self, name: &str) -> Option<&SpockFn> {
        self.fns.iter().find(|f| f.name == name)
    }
}

impl SpockTable {
    pub fn find_column(&self, name: &str) -> Option<&SpockColumn> {
        self.columns.iter().find(|c| c.name == name)
    }
}

fn column_base(ty: &Value) -> Option<String> {
    match ty.get("kind")?.as_str()? {
        "ref" => Some(ty.get("table")?.as_str()?.to_string()),
        "set" => Some("enum".to_string()),
        scalar => Some(scalar.to_string()),
    }
}

/// Contract JSON → schema. On shape mismatch, return a problem list instead of guessing.
pub fn extract_contract(source: &str) -> Result<SpockSchema, Vec<String>> {
    let root: Value = match serde_json::from_str(source) {
        Ok(v) => v,
        Err(e) => return Err(vec![format!("contract is not valid JSON: {e}")]),
    };
    let mut problems = Vec::new();
    let mut schema = SpockSchema::default();

    match root.get("tables").and_then(Value::as_array) {
        None => problems.push("contract has no `tables` array".to_string()),
        Some(tables) => {
            for t in tables {
                let Some(name) = t.get("name").and_then(Value::as_str) else {
                    problems.push("table entry without a name".to_string());
                    continue;
                };
                let keys: Vec<&str> = t
                    .get("key")
                    .and_then(Value::as_array)
                    .map(|k| k.iter().filter_map(Value::as_str).collect())
                    .unwrap_or_default();
                let mut table = SpockTable {
                    name: name.to_string(),
                    columns: Vec::new(),
                };
                for f in t
                    .get("fields")
                    .and_then(Value::as_array)
                    .unwrap_or(&Vec::new())
                {
                    let Some(col_name) = f.get("name").and_then(Value::as_str) else {
                        problems.push(format!("table `{name}`: field without a name"));
                        continue;
                    };
                    let Some(base) = f.get("type").and_then(column_base) else {
                        problems.push(format!(
                            "table `{name}`: field `{col_name}` has an unrecognized type"
                        ));
                        continue;
                    };
                    table.columns.push(SpockColumn {
                        name: col_name.to_string(),
                        base,
                        key: keys.contains(&col_name),
                    });
                }
                schema.tables.push(table);
            }
        }
    }

    for e in root
        .get("errors")
        .and_then(Value::as_array)
        .unwrap_or(&Vec::new())
    {
        if let Some(code) = e.get("code").and_then(Value::as_str) {
            schema.errors.push(code.to_string());
        }
    }

    match root.get("fns").and_then(Value::as_array) {
        None => problems.push("contract has no `fns` array".to_string()),
        Some(fns) => {
            for f in fns {
                let Some(name) = f.get("name").and_then(Value::as_str) else {
                    problems.push("fn entry without a name".to_string());
                    continue;
                };
                let errors = f
                    .get("errors")
                    .and_then(Value::as_array)
                    .map(|a| {
                        a.iter()
                            .filter_map(Value::as_str)
                            .map(str::to_string)
                            .collect()
                    })
                    .unwrap_or_default();
                let mutating = !f.get("readonly").and_then(Value::as_bool).unwrap_or(false);
                schema.fns.push(SpockFn {
                    name: name.to_string(),
                    errors,
                    mutating,
                });
            }
        }
    }

    if problems.is_empty() {
        Ok(schema)
    } else {
        Err(problems)
    }
}

// ===== CLI seam: typed contract + declaration path → provider module =====

/// `spock gen provider <program> --app <decl>`: generate the provider tables module
/// from the contract (compiler-owned) and the app declaration; problems merge into one failure.
pub fn generate_from_contract<C: ::serde::Serialize>(
    contract: &C,
    app_declaration: &std::path::Path,
) -> Result<String, String> {
    let json = serde_json::to_string(contract)
        .map_err(|e| format!("contract serialization failed: {e}"))?;
    let schema = extract_contract(&json).map_err(|p| p.join("\n"))?;
    let source = std::fs::read_to_string(app_declaration)
        .map_err(|e| format!("cannot read {}: {e}", app_declaration.display()))?;
    let file = parse(&source).map_err(|e| e.to_string())?;
    let problems = validate_against(&file, &schema);
    if !problems.is_empty() {
        return Err(problems.join("\n"));
    }
    generate_provider_module(&file, &schema).map_err(|p| p.join("\n"))
}

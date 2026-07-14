use std::path::Path;

use toml::Table;

use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, ProjectResult};
use crate::path::NormalizedRelativePath;

pub const MANIFEST_FILE: &str = "spock.toml";
pub const MANIFEST_VERSION: i64 = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectName(String);

impl ProjectName {
    pub fn parse(value: &str) -> Result<Self, String> {
        if value.is_empty() {
            return Err("project name must not be empty".to_string());
        }
        if value.trim() != value {
            return Err("project name must not begin or end with whitespace".to_string());
        }
        if value.chars().any(char::is_control) {
            return Err("project name must not contain control characters".to_string());
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendConfig {
    root: NormalizedRelativePath,
    entry: NormalizedRelativePath,
}

impl BackendConfig {
    pub fn root(&self) -> &NormalizedRelativePath {
        &self.root
    }

    pub fn entry(&self) -> &NormalizedRelativePath {
        &self.entry
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientConfig {
    root: NormalizedRelativePath,
}

impl ClientConfig {
    pub fn root(&self) -> &NormalizedRelativePath {
        &self.root
    }
}

/// The complete version-1 framework topology manifest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectManifest {
    version: u32,
    project: ProjectName,
    backend: BackendConfig,
    client: Option<ClientConfig>,
}

impl ProjectManifest {
    pub fn version(&self) -> u32 {
        self.version
    }

    pub fn project(&self) -> &ProjectName {
        &self.project
    }

    pub fn backend(&self) -> &BackendConfig {
        &self.backend
    }

    pub fn client(&self) -> Option<&ClientConfig> {
        self.client.as_ref()
    }

    pub fn new(
        project_name: &str,
        backend_root: &str,
        backend_entry: &str,
        client_root: Option<&str>,
    ) -> ProjectResult<Self> {
        let mut diagnostics = Diagnostics::new();
        let project = validate_project_name(project_name, None, &mut diagnostics);
        let backend_root = validate_root_path(backend_root, "backend.root", None, &mut diagnostics);
        let backend_entry =
            validate_file_path(backend_entry, "backend.entry", None, &mut diagnostics);
        if let Some(entry) = &backend_entry {
            if entry.extension() != Some("spock") {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::InvalidManifestPath,
                    "`backend.entry` must name a `.spock` file",
                ));
            }
        }
        let client_root =
            client_root.map(|root| validate_root_path(root, "client.root", None, &mut diagnostics));

        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        Ok(Self {
            version: MANIFEST_VERSION as u32,
            project: project.expect("validated project name"),
            backend: BackendConfig {
                root: backend_root.expect("validated backend root"),
                entry: backend_entry.expect("validated backend entry"),
            },
            client: client_root.map(|root| ClientConfig {
                root: root.expect("validated client root"),
            }),
        })
    }

    /// Render the canonical manifest form used by scaffold and adoption plans.
    pub fn to_toml_string(&self) -> String {
        let mut rendered = format!(
            "version = {}\n\n[project]\nname = {}\n\n[backend]\nroot = {}\nentry = {}\n",
            self.version,
            toml_string(self.project.as_str()),
            toml_string(self.backend.root.as_str()),
            toml_string(self.backend.entry.as_str()),
        );
        if let Some(client) = &self.client {
            rendered.push_str(&format!(
                "\n[client]\nroot = {}\n",
                toml_string(client.root.as_str())
            ));
        }
        rendered
    }
}

pub fn parse_manifest(source: &str) -> ProjectResult<ProjectManifest> {
    parse_manifest_at(source, None)
}

pub fn parse_manifest_file(source: &str, path: &Path) -> ProjectResult<ProjectManifest> {
    parse_manifest_at(source, Some(path))
}

fn parse_manifest_at(source: &str, path: Option<&Path>) -> ProjectResult<ProjectManifest> {
    let table = toml::from_str::<Table>(source).map_err(|error| {
        let mut diagnostic =
            Diagnostic::new(DiagnosticCode::TomlSyntax, format!("invalid TOML: {error}"));
        if let Some(path) = path {
            diagnostic = diagnostic.at_path(path);
        }
        if let Some(span) = error.span() {
            diagnostic = diagnostic.with_span(span);
        }
        Diagnostics::one(diagnostic)
    })?;

    let mut diagnostics = Diagnostics::new();
    reject_unknown_fields(
        &table,
        &["version", "project", "backend", "client"],
        "",
        path,
        &mut diagnostics,
    );

    let version = integer_field(&table, "version", "version", path, &mut diagnostics);
    if let Some(version) = version {
        if version != MANIFEST_VERSION {
            push_at(
                &mut diagnostics,
                Diagnostic::new(
                    DiagnosticCode::UnsupportedVersion,
                    format!(
                        "unsupported manifest version {version}; this tool supports version {MANIFEST_VERSION}"
                    ),
                ),
                path,
            );
        }
    }

    let project_table = required_table(&table, "project", path, &mut diagnostics);
    let backend_table = required_table(&table, "backend", path, &mut diagnostics);
    let client_table = optional_table(&table, "client", path, &mut diagnostics);

    let project = project_table.and_then(|section| {
        reject_unknown_fields(section, &["name"], "project", path, &mut diagnostics);
        string_field(section, "name", "project.name", path, &mut diagnostics)
            .and_then(|name| validate_project_name(&name, path, &mut diagnostics))
    });

    let (backend_root, backend_entry) = if let Some(section) = backend_table {
        reject_unknown_fields(
            section,
            &["root", "entry"],
            "backend",
            path,
            &mut diagnostics,
        );
        let root = string_field(section, "root", "backend.root", path, &mut diagnostics)
            .and_then(|root| validate_root_path(&root, "backend.root", path, &mut diagnostics));
        let entry = string_field(section, "entry", "backend.entry", path, &mut diagnostics)
            .and_then(|entry| validate_file_path(&entry, "backend.entry", path, &mut diagnostics));
        if let Some(entry) = &entry {
            if entry.extension() != Some("spock") {
                push_at(
                    &mut diagnostics,
                    Diagnostic::new(
                        DiagnosticCode::InvalidManifestPath,
                        "`backend.entry` must name a `.spock` file",
                    ),
                    path,
                );
            }
        }
        (root, entry)
    } else {
        (None, None)
    };

    let client_root = client_table.map(|section| {
        reject_unknown_fields(section, &["root"], "client", path, &mut diagnostics);
        string_field(section, "root", "client.root", path, &mut diagnostics)
            .and_then(|root| validate_root_path(&root, "client.root", path, &mut diagnostics))
    });

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(ProjectManifest {
        version: version.expect("validated version") as u32,
        project: project.expect("validated project section"),
        backend: BackendConfig {
            root: backend_root.expect("validated backend root"),
            entry: backend_entry.expect("validated backend entry"),
        },
        client: client_root.map(|root| ClientConfig {
            root: root.expect("validated client root"),
        }),
    })
}

fn required_table<'a>(
    root: &'a Table,
    field: &str,
    path: Option<&Path>,
    diagnostics: &mut Diagnostics,
) -> Option<&'a Table> {
    match root.get(field) {
        Some(toml::Value::Table(table)) => Some(table),
        Some(_) => {
            push_at(
                diagnostics,
                Diagnostic::new(
                    DiagnosticCode::WrongType,
                    format!("`{field}` must be a table"),
                ),
                path,
            );
            None
        }
        None => {
            push_at(
                diagnostics,
                Diagnostic::new(
                    DiagnosticCode::MissingField,
                    format!("missing required `[{field}]` table"),
                ),
                path,
            );
            None
        }
    }
}

fn optional_table<'a>(
    root: &'a Table,
    field: &str,
    path: Option<&Path>,
    diagnostics: &mut Diagnostics,
) -> Option<&'a Table> {
    match root.get(field) {
        Some(toml::Value::Table(table)) => Some(table),
        Some(_) => {
            push_at(
                diagnostics,
                Diagnostic::new(
                    DiagnosticCode::WrongType,
                    format!("`{field}` must be a table"),
                ),
                path,
            );
            None
        }
        None => None,
    }
}

fn string_field(
    table: &Table,
    field: &str,
    qualified: &str,
    path: Option<&Path>,
    diagnostics: &mut Diagnostics,
) -> Option<String> {
    match table.get(field) {
        Some(toml::Value::String(value)) => Some(value.clone()),
        Some(_) => {
            push_at(
                diagnostics,
                Diagnostic::new(
                    DiagnosticCode::WrongType,
                    format!("`{qualified}` must be a string"),
                ),
                path,
            );
            None
        }
        None => {
            push_at(
                diagnostics,
                Diagnostic::new(
                    DiagnosticCode::MissingField,
                    format!("missing required `{qualified}`"),
                ),
                path,
            );
            None
        }
    }
}

fn integer_field(
    table: &Table,
    field: &str,
    qualified: &str,
    path: Option<&Path>,
    diagnostics: &mut Diagnostics,
) -> Option<i64> {
    match table.get(field) {
        Some(toml::Value::Integer(value)) => Some(*value),
        Some(_) => {
            push_at(
                diagnostics,
                Diagnostic::new(
                    DiagnosticCode::WrongType,
                    format!("`{qualified}` must be an integer"),
                ),
                path,
            );
            None
        }
        None => {
            push_at(
                diagnostics,
                Diagnostic::new(
                    DiagnosticCode::MissingField,
                    format!("missing required `{qualified}`"),
                ),
                path,
            );
            None
        }
    }
}

fn reject_unknown_fields(
    table: &Table,
    accepted: &[&str],
    section: &str,
    path: Option<&Path>,
    diagnostics: &mut Diagnostics,
) {
    let mut unknown = table
        .keys()
        .filter(|field| !accepted.contains(&field.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    unknown.sort();
    for field in unknown {
        let qualified = if section.is_empty() {
            field
        } else {
            format!("{section}.{field}")
        };
        push_at(
            diagnostics,
            Diagnostic::new(
                DiagnosticCode::UnknownField,
                format!("unknown manifest field `{qualified}`"),
            ),
            path,
        );
    }
}

fn validate_project_name(
    value: &str,
    path: Option<&Path>,
    diagnostics: &mut Diagnostics,
) -> Option<ProjectName> {
    match ProjectName::parse(value) {
        Ok(name) => Some(name),
        Err(message) => {
            push_at(
                diagnostics,
                Diagnostic::new(DiagnosticCode::InvalidProjectName, message),
                path,
            );
            None
        }
    }
}

fn validate_root_path(
    value: &str,
    field: &str,
    path: Option<&Path>,
    diagnostics: &mut Diagnostics,
) -> Option<NormalizedRelativePath> {
    match NormalizedRelativePath::root(value) {
        Ok(path_value) => Some(path_value),
        Err(error) => {
            push_at(
                diagnostics,
                Diagnostic::new(
                    DiagnosticCode::InvalidManifestPath,
                    format!("invalid `{field}`: {error}"),
                ),
                path,
            );
            None
        }
    }
}

fn validate_file_path(
    value: &str,
    field: &str,
    path: Option<&Path>,
    diagnostics: &mut Diagnostics,
) -> Option<NormalizedRelativePath> {
    match NormalizedRelativePath::file(value) {
        Ok(path_value) => Some(path_value),
        Err(error) => {
            push_at(
                diagnostics,
                Diagnostic::new(
                    DiagnosticCode::InvalidManifestPath,
                    format!("invalid `{field}`: {error}"),
                ),
                path,
            );
            None
        }
    }
}

fn push_at(diagnostics: &mut Diagnostics, mut diagnostic: Diagnostic, path: Option<&Path>) {
    if let Some(path) = path {
        diagnostic = diagnostic.at_path(path);
    }
    diagnostics.push(diagnostic);
}

fn toml_string(value: &str) -> String {
    let mut rendered = String::with_capacity(value.len() + 2);
    rendered.push('"');
    for character in value.chars() {
        match character {
            '"' => rendered.push_str("\\\""),
            '\\' => rendered.push_str("\\\\"),
            '\n' => rendered.push_str("\\n"),
            '\r' => rendered.push_str("\\r"),
            '\t' => rendered.push_str("\\t"),
            other => rendered.push(other),
        }
    }
    rendered.push('"');
    rendered
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINIMAL: &str = r#"version = 1

[project]
name = "demo"

[backend]
root = "backend"
entry = "app.spock"
"#;

    #[test]
    fn parses_the_strict_version_one_shape() {
        let manifest = parse_manifest(MINIMAL).unwrap();
        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.project.as_str(), "demo");
        assert_eq!(manifest.backend.root.as_str(), "backend");
        assert_eq!(manifest.backend.entry.as_str(), "app.spock");
        assert!(manifest.client.is_none());

        let with_client =
            parse_manifest(&format!("{MINIMAL}\n[client]\nroot = \"client\"\n")).unwrap();
        assert_eq!(with_client.client.unwrap().root.as_str(), "client");
    }

    #[test]
    fn rejects_unknown_fields_at_every_level_in_sorted_order() {
        let source = r#"version = 1
z = true
a = true
[project]
name = "demo"
other = true
[backend]
root = "backend"
entry = "app.spock"
extra = true
"#;
        let diagnostics = parse_manifest(source).unwrap_err().into_vec();
        let messages = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();
        assert_eq!(
            messages,
            [
                "unknown manifest field `a`",
                "unknown manifest field `z`",
                "unknown manifest field `project.other`",
                "unknown manifest field `backend.extra`",
            ]
        );
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == DiagnosticCode::UnknownField));
    }

    #[test]
    fn reports_missing_types_versions_and_paths_structurally() {
        let diagnostics = parse_manifest(
            r#"version = 2
[project]
name = 4
[backend]
root = "../outside"
entry = "app.txt"
[client]
unknown = true
"#,
        )
        .unwrap_err();
        let codes = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>();
        assert!(codes.contains(&DiagnosticCode::UnsupportedVersion));
        assert!(codes.contains(&DiagnosticCode::WrongType));
        assert!(codes.contains(&DiagnosticCode::InvalidManifestPath));
        assert!(codes.contains(&DiagnosticCode::UnknownField));
        assert!(codes.contains(&DiagnosticCode::MissingField));
    }

    #[test]
    fn canonical_render_round_trips_and_escapes_names() {
        let manifest =
            ProjectManifest::new("a \\\"quoted\\\" project", ".", "app.spock", Some("client"))
                .unwrap();
        let rendered = manifest.to_toml_string();
        assert!(rendered.ends_with('\n'));
        assert_eq!(parse_manifest(&rendered).unwrap(), manifest);
    }

    #[test]
    fn syntax_diagnostic_carries_a_source_span_and_path() {
        let path = Path::new("/project/spock.toml");
        let diagnostic = parse_manifest_file("version = [", path)
            .unwrap_err()
            .into_vec()
            .remove(0);
        assert_eq!(diagnostic.code, DiagnosticCode::TomlSyntax);
        assert_eq!(diagnostic.path.as_deref(), Some(path));
        assert!(diagnostic.span.is_some());
    }
}

use std::fmt;
use std::ops::Range;
use std::path::PathBuf;

/// Stable diagnostic categories emitted by the project layer.
///
/// The human text may become more helpful over time; callers should branch on
/// this code rather than parsing that text.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum DiagnosticCode {
    TomlSyntax,
    MissingField,
    UnknownField,
    WrongType,
    UnsupportedVersion,
    InvalidProjectName,
    InvalidManifestPath,
    Io,
    ProjectNotFound,
    UnsupportedTarget,
    PathEscape,
    MissingInput,
    WrongEntryKind,
    AlreadyProject,
    AmbiguousBackend,
    AmbiguousClient,
    PlanConflict,
    UnsafeSymlink,
    InvalidTemplate,
}

impl DiagnosticCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TomlSyntax => "SPP001",
            Self::MissingField => "SPP002",
            Self::UnknownField => "SPP003",
            Self::WrongType => "SPP004",
            Self::UnsupportedVersion => "SPP005",
            Self::InvalidProjectName => "SPP006",
            Self::InvalidManifestPath => "SPP007",
            Self::Io => "SPP008",
            Self::ProjectNotFound => "SPP009",
            Self::UnsupportedTarget => "SPP010",
            Self::PathEscape => "SPP011",
            Self::MissingInput => "SPP012",
            Self::WrongEntryKind => "SPP013",
            Self::AlreadyProject => "SPP014",
            Self::AmbiguousBackend => "SPP015",
            Self::AmbiguousClient => "SPP016",
            Self::PlanConflict => "SPP017",
            Self::UnsafeSymlink => "SPP018",
            Self::InvalidTemplate => "SPP019",
        }
    }
}

impl fmt::Display for DiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One structured project diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Diagnostic {
    pub code: DiagnosticCode,
    pub message: String,
    pub path: Option<PathBuf>,
    pub span: Option<Range<usize>>,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn new(code: DiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            path: None,
            span: None,
            notes: Vec::new(),
        }
    }

    pub fn at_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }

    pub fn with_span(mut self, span: Range<usize>) -> Self {
        self.span = Some(span);
        self
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: ", self.code)?;
        if let Some(path) = &self.path {
            write!(formatter, "{}: ", path.display())?;
        }
        formatter.write_str(&self.message)?;
        for note in &self.notes {
            write!(formatter, "\n  note: {note}")?;
        }
        Ok(())
    }
}

/// A deterministic collection of diagnostics from one operation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Diagnostics(Vec<Diagnostic>);

impl Diagnostics {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn one(diagnostic: Diagnostic) -> Self {
        Self(vec![diagnostic])
    }

    pub fn push(&mut self, diagnostic: Diagnostic) {
        self.0.push(diagnostic);
    }

    pub fn extend(&mut self, diagnostics: impl IntoIterator<Item = Diagnostic>) {
        self.0.extend(diagnostics);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Diagnostic> {
        self.0.iter()
    }

    pub fn into_vec(self) -> Vec<Diagnostic> {
        self.0
    }
}

impl From<Diagnostic> for Diagnostics {
    fn from(diagnostic: Diagnostic) -> Self {
        Self::one(diagnostic)
    }
}

impl IntoIterator for Diagnostics {
    type Item = Diagnostic;
    type IntoIter = std::vec::IntoIter<Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<'a> IntoIterator for &'a Diagnostics {
    type Item = &'a Diagnostic;
    type IntoIter = std::slice::Iter<'a, Diagnostic>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl fmt::Display for Diagnostics {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, diagnostic) in self.0.iter().enumerate() {
            if index != 0 {
                formatter.write_str("\n")?;
            }
            diagnostic.fmt(formatter)?;
        }
        Ok(())
    }
}

impl std::error::Error for Diagnostics {}

pub type ProjectResult<T> = Result<T, Diagnostics>;

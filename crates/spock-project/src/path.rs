use std::fmt;
use std::fs;
use std::path::{Component, Path, PathBuf};

use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, ProjectResult};

/// A portable, normalized path stored relative to a Spock project root.
///
/// Manifest paths always use `/`, even on Windows. `.` is the only spelling
/// for the project root; redundant separators and dot components are rejected
/// so one logical input has one manifest spelling.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct NormalizedRelativePath(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PathValidationError(String);

impl PathValidationError {
    pub fn message(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for PathValidationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PathValidationError {}

impl NormalizedRelativePath {
    /// Parse a manifest root. The special value `.` names the project root.
    pub fn root(value: &str) -> Result<Self, PathValidationError> {
        Self::parse(value, true)
    }

    /// Parse a non-root relative path, such as a backend entry or template
    /// file. `.` is not a file path.
    pub fn file(value: &str) -> Result<Self, PathValidationError> {
        Self::parse(value, false)
    }

    fn parse(value: &str, allow_dot: bool) -> Result<Self, PathValidationError> {
        if value == "." {
            return if allow_dot {
                Ok(Self(value.to_string()))
            } else {
                Err(PathValidationError(
                    "`.` names a directory, not a file".to_string(),
                ))
            };
        }
        if value.is_empty() {
            return Err(PathValidationError("path must not be empty".to_string()));
        }
        if value.starts_with('/') {
            return Err(PathValidationError(
                "path must be relative to the project root".to_string(),
            ));
        }
        if value.contains('\\') {
            return Err(PathValidationError(
                "use `/` in manifest paths; backslashes are not portable".to_string(),
            ));
        }
        if value.contains('\0') {
            return Err(PathValidationError("path contains NUL".to_string()));
        }

        let segments = value.split('/').collect::<Vec<_>>();
        if segments
            .first()
            .is_some_and(|segment| is_windows_drive_prefix(segment))
        {
            return Err(PathValidationError(
                "path must not contain a Windows drive prefix".to_string(),
            ));
        }
        for segment in &segments {
            if segment.is_empty() {
                return Err(PathValidationError(
                    "path contains an empty segment or trailing `/`".to_string(),
                ));
            }
            if *segment == "." {
                return Err(PathValidationError(
                    "path contains a redundant `.` segment".to_string(),
                ));
            }
            if *segment == ".." {
                return Err(PathValidationError(
                    "path must not contain `..` or escape its base directory".to_string(),
                ));
            }
            if segment.chars().any(char::is_control) {
                return Err(PathValidationError(
                    "path contains a control character".to_string(),
                ));
            }
            if let Some(character) = segment
                .chars()
                .find(|character| matches!(character, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
            {
                return Err(PathValidationError(format!(
                    "path segment `{segment}` contains Windows-reserved character `{character}`"
                )));
            }
            if segment.ends_with('.') {
                return Err(PathValidationError(format!(
                    "path segment `{segment}` must not end with `.`; Windows removes trailing dots"
                )));
            }
            if segment.ends_with(' ') {
                return Err(PathValidationError(format!(
                    "path segment `{segment}` must not end with a space; Windows removes trailing spaces"
                )));
            }
            if is_windows_device_name(segment) {
                return Err(PathValidationError(format!(
                    "path segment `{segment}` is a reserved Windows device name"
                )));
            }
        }

        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn as_path(&self) -> &Path {
        Path::new(&self.0)
    }

    pub fn is_project_root(&self) -> bool {
        self.0 == "."
    }

    pub fn join(&self, child: &Self) -> Self {
        match (self.is_project_root(), child.is_project_root()) {
            (true, true) => Self(".".to_string()),
            (true, false) => child.clone(),
            (false, true) => self.clone(),
            (false, false) => Self(format!("{}/{}", self.0, child.0)),
        }
    }

    pub fn parent(&self) -> Self {
        if self.is_project_root() || !self.0.contains('/') {
            return Self(".".to_string());
        }
        Self(
            self.0
                .rsplit_once('/')
                .expect("contains slash")
                .0
                .to_string(),
        )
    }

    pub fn file_name(&self) -> Option<&str> {
        (!self.is_project_root())
            .then(|| self.0.rsplit('/').next())
            .flatten()
    }

    pub fn extension(&self) -> Option<&str> {
        self.file_name()
            .and_then(|name| name.rsplit_once('.').map(|(_, extension)| extension))
    }
}

impl fmt::Display for NormalizedRelativePath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn is_windows_drive_prefix(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    bytes.len() == 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

fn is_windows_device_name(segment: &str) -> bool {
    // Match the device-name guard used by the Windows capability adapter. The
    // stem is the portion before the first dot, and Windows-compatible opens
    // trim whitespace at the end of that stem before comparing it.
    let stem = segment
        .split_once('.')
        .map_or(segment, |(stem, _)| stem)
        .trim_end()
        .to_uppercase();
    matches!(
        stem.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM0"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "COM¹"
            | "COM²"
            | "COM³"
            | "LPT0"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
            | "LPT¹"
            | "LPT²"
            | "LPT³"
    )
}

/// One logical project-relative path and its canonical absolute resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContainedPath {
    relative: NormalizedRelativePath,
    absolute: PathBuf,
}

impl ContainedPath {
    pub fn relative(&self) -> &NormalizedRelativePath {
        &self.relative
    }

    pub fn absolute(&self) -> &Path {
        &self.absolute
    }
}

/// Resolve an existing or not-yet-created relative path without allowing an
/// existing symlink ancestor to leave `root`.
pub fn resolve_contained(
    root: &Path,
    relative: &NormalizedRelativePath,
) -> ProjectResult<ContainedPath> {
    let canonical_root = fs::canonicalize(root).map_err(|error| {
        Diagnostics::one(
            Diagnostic::new(
                DiagnosticCode::Io,
                format!("could not resolve project root: {error}"),
            )
            .at_path(root),
        )
    })?;
    if !canonical_root.is_dir() {
        return Err(Diagnostic::new(
            DiagnosticCode::WrongEntryKind,
            "project root is not a directory",
        )
        .at_path(root)
        .into());
    }

    let candidate = canonical_root.join(relative.as_path());
    let absolute = canonicalize_with_missing_tail(&candidate)
        .map_err(|diagnostic| Diagnostics::one(diagnostic.at_path(candidate.clone())))?;
    if !absolute.starts_with(&canonical_root) {
        return Err(Diagnostic::new(
            DiagnosticCode::PathEscape,
            format!(
                "`{relative}` resolves outside project root {}",
                canonical_root.display()
            ),
        )
        .at_path(candidate)
        .into());
    }

    Ok(ContainedPath {
        relative: relative.clone(),
        absolute,
    })
}

fn canonicalize_with_missing_tail(candidate: &Path) -> Result<PathBuf, Diagnostic> {
    let mut cursor = candidate.to_path_buf();
    let mut tail = Vec::new();
    loop {
        match fs::symlink_metadata(&cursor) {
            Ok(_) => {
                let mut resolved = fs::canonicalize(&cursor).map_err(|error| {
                    Diagnostic::new(
                        DiagnosticCode::Io,
                        format!("could not resolve path: {error}"),
                    )
                })?;
                for segment in tail.iter().rev() {
                    resolved.push(segment);
                }
                return Ok(resolved);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let Some(name) = cursor.file_name() else {
                    return Err(Diagnostic::new(
                        DiagnosticCode::Io,
                        "could not find an existing ancestor for path",
                    ));
                };
                tail.push(name.to_os_string());
                let Some(parent) = cursor.parent() else {
                    return Err(Diagnostic::new(
                        DiagnosticCode::Io,
                        "could not find an existing ancestor for path",
                    ));
                };
                cursor = parent.to_path_buf();
            }
            Err(error) => {
                return Err(Diagnostic::new(
                    DiagnosticCode::Io,
                    format!("could not inspect path: {error}"),
                ));
            }
        }
    }
}

/// Canonicalize an existing directory for project discovery.
pub(crate) fn canonical_directory(path: &Path) -> ProjectResult<PathBuf> {
    let canonical = fs::canonicalize(path).map_err(|error| {
        Diagnostics::one(
            Diagnostic::new(
                DiagnosticCode::Io,
                format!("could not resolve directory: {error}"),
            )
            .at_path(path),
        )
    })?;
    if !canonical.is_dir() {
        return Err(
            Diagnostic::new(DiagnosticCode::WrongEntryKind, "expected a directory")
                .at_path(path)
                .into(),
        );
    }
    Ok(canonical)
}

/// Make a command target absolute and lexically normalized. Existing paths
/// are canonicalized so aliases resolve deterministically.
pub(crate) fn absolute_target(cwd: &Path, target: &Path) -> ProjectResult<PathBuf> {
    let joined = if target.is_absolute() {
        target.to_path_buf()
    } else {
        cwd.join(target)
    };
    if joined.exists() {
        return fs::canonicalize(&joined).map_err(|error| {
            Diagnostics::one(
                Diagnostic::new(
                    DiagnosticCode::Io,
                    format!("could not resolve target: {error}"),
                )
                .at_path(joined),
            )
        });
    }
    Ok(lexically_normalize_absolute(&joined))
}

fn lexically_normalize_absolute(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portable_relative_paths_have_one_spelling() {
        assert_eq!(NormalizedRelativePath::root(".").unwrap().as_str(), ".");
        assert_eq!(
            NormalizedRelativePath::file("schema/app.spock")
                .unwrap()
                .as_str(),
            "schema/app.spock"
        );

        for invalid in [
            "",
            "/tmp/app.spock",
            "C:/app.spock",
            "../app.spock",
            "a/../b",
            "a//b",
            "a/./b",
            "a\\b",
            "a/",
        ] {
            assert!(
                NormalizedRelativePath::file(invalid).is_err(),
                "accepted {invalid:?}"
            );
        }
        assert!(NormalizedRelativePath::file(".").is_err());
    }

    #[test]
    fn portable_paths_reject_windows_aliases_in_every_segment() {
        for invalid in [
            "schema:shadow/app.spock",
            "schema/app.spock:shadow",
            "schema./app.spock",
            "schema/app.spock.",
            "schema /app.spock",
            "schema/app.spock ",
            "schema/app<.spock",
            "schema/app>.spock",
            "schema/app\".spock",
            "schema/app|.spock",
            "schema/app?.spock",
            "schema/app*.spock",
            "schema/CON/app.spock",
            "schema/prn.txt",
            "schema/AUX",
            "schema/nul.txt",
            "schema/COM0",
            "schema/COM1",
            "schema/lpt0.log",
            "schema/lpt9.log",
            "schema/COM¹",
            "schema/lpt².log",
            "schema/CON .txt",
            "schema/com³ .log",
        ] {
            assert!(
                NormalizedRelativePath::file(invalid).is_err(),
                "accepted Windows-ambiguous path {invalid:?}"
            );
        }

        for valid in [
            "evidence.uhura",
            "host.toml",
            "machine.uhura",
            "ui.uhura",
            "uhura.toml",
        ] {
            assert_eq!(NormalizedRelativePath::file(valid).unwrap().as_str(), valid);
        }
    }

    #[test]
    fn normalized_paths_join_and_parent_without_platform_dependence() {
        let client = NormalizedRelativePath::root("client").unwrap();
        let manifest = NormalizedRelativePath::file("nested/uhura.toml").unwrap();
        assert_eq!(client.join(&manifest).as_str(), "client/nested/uhura.toml");
        assert_eq!(manifest.parent().as_str(), "nested");
        assert_eq!(client.parent().as_str(), ".");
    }
}

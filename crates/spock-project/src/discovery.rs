use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, ProjectResult};
use crate::manifest::MANIFEST_FILE;
use crate::path::{absolute_target, canonical_directory};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectRoot {
    root: PathBuf,
}

impl ProjectRoot {
    pub fn path(&self) -> &Path {
        &self.root
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.root.join(MANIFEST_FILE)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResolvedTarget {
    SpockFile(PathBuf),
    Project(ProjectRoot),
}

/// Find the nearest `spock.toml`, starting at a directory (or the parent of an
/// existing file) and walking toward the filesystem root.
pub fn discover_project_root(start: &Path) -> ProjectResult<ProjectRoot> {
    let start_directory = if start.is_file() {
        start.parent().unwrap_or(start)
    } else {
        start
    };
    let canonical = canonical_directory(start_directory)?;
    let mut searched = Vec::new();
    for ancestor in canonical.ancestors() {
        searched.push(ancestor.to_path_buf());
        if fs::symlink_metadata(ancestor.join(MANIFEST_FILE)).is_ok() {
            return Ok(ProjectRoot {
                root: ancestor.to_path_buf(),
            });
        }
    }

    let mut diagnostic = Diagnostic::new(
        DiagnosticCode::ProjectNotFound,
        format!(
            "could not find `{MANIFEST_FILE}` from {} or any parent directory",
            canonical.display()
        ),
    )
    .at_path(canonical);
    for directory in searched {
        diagnostic = diagnostic.with_note(format!("searched {}", directory.display()));
    }
    Err(diagnostic.into())
}

/// Resolve the CLI's polymorphic target without reading or interpreting either
/// source language.
///
/// An explicit `.spock` spelling always selects file mode, even when the file
/// does not exist yet. An omitted target or directory selects the nearest
/// enclosing project. An explicit `spock.toml` selects exactly its parent.
pub fn resolve_target(target: Option<&Path>, cwd: &Path) -> ProjectResult<ResolvedTarget> {
    let canonical_cwd = canonical_directory(cwd)?;
    let Some(target) = target else {
        return discover_project_root(&canonical_cwd).map(ResolvedTarget::Project);
    };

    if target.extension().and_then(|extension| extension.to_str()) == Some("spock") {
        return absolute_target(&canonical_cwd, target).map(ResolvedTarget::SpockFile);
    }

    if target.file_name().and_then(|name| name.to_str()) == Some(MANIFEST_FILE) {
        // Resolve the parent, not the manifest itself. This deliberately keeps
        // a final manifest symlink visible so `load_project` can reject it
        // instead of silently changing the selected project root.
        let requested_parent = target.parent().unwrap_or_else(|| Path::new("."));
        let parent = absolute_target(&canonical_cwd, requested_parent)?;
        let absolute = parent.join(MANIFEST_FILE);
        let metadata = fs::symlink_metadata(&absolute).map_err(|error| {
            Diagnostics::one(
                Diagnostic::new(
                    DiagnosticCode::MissingInput,
                    format!("could not read explicit project manifest: {error}"),
                )
                .at_path(&absolute),
            )
        })?;
        if metadata.file_type().is_dir() {
            return Err(Diagnostic::new(
                DiagnosticCode::WrongEntryKind,
                "explicit project manifest is a directory",
            )
            .at_path(absolute)
            .into());
        }
        return Ok(ResolvedTarget::Project(ProjectRoot { root: parent }));
    }

    let absolute = absolute_target(&canonical_cwd, target)?;
    match fs::metadata(&absolute) {
        Ok(metadata) if metadata.is_dir() => {
            discover_project_root(&absolute).map(ResolvedTarget::Project)
        }
        Ok(_) => Err(Diagnostic::new(
            DiagnosticCode::UnsupportedTarget,
            format!(
                "target is neither a `.spock` file, `{MANIFEST_FILE}`, nor a project directory"
            ),
        )
        .at_path(absolute)
        .into()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Err(Diagnostic::new(
            DiagnosticCode::UnsupportedTarget,
            "target does not exist and is not a `.spock` file",
        )
        .at_path(absolute)
        .into()),
        Err(error) => Err(Diagnostic::new(
            DiagnosticCode::Io,
            format!("could not inspect target: {error}"),
        )
        .at_path(absolute)
        .into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn nearest_manifest_wins_at_nested_project_boundaries() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join(MANIFEST_FILE), "").unwrap();
        let nested = temp.path().join("a/nested");
        fs::create_dir_all(nested.join("src/deep")).unwrap();
        fs::write(nested.join(MANIFEST_FILE), "").unwrap();

        let root = discover_project_root(&nested.join("src/deep")).unwrap();
        assert_eq!(root.path(), fs::canonicalize(nested).unwrap());
    }

    #[test]
    fn omitted_directory_manifest_and_file_targets_are_unambiguous() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join(MANIFEST_FILE), "").unwrap();
        fs::create_dir(temp.path().join("child")).unwrap();

        assert!(matches!(
            resolve_target(None, &temp.path().join("child")).unwrap(),
            ResolvedTarget::Project(_)
        ));
        assert!(matches!(
            resolve_target(Some(Path::new("child")), temp.path()).unwrap(),
            ResolvedTarget::Project(_)
        ));
        assert!(matches!(
            resolve_target(Some(Path::new(MANIFEST_FILE)), temp.path()).unwrap(),
            ResolvedTarget::Project(_)
        ));
        let target = resolve_target(Some(Path::new("missing.spock")), temp.path()).unwrap();
        assert_eq!(
            target,
            ResolvedTarget::SpockFile(fs::canonicalize(temp.path()).unwrap().join("missing.spock"))
        );
    }

    #[test]
    fn project_not_found_reports_every_searched_directory() {
        let temp = tempdir().unwrap();
        let nested = temp.path().join("a/b");
        fs::create_dir_all(&nested).unwrap();
        let diagnostic = discover_project_root(&nested)
            .unwrap_err()
            .into_vec()
            .remove(0);
        assert_eq!(diagnostic.code, DiagnosticCode::ProjectNotFound);
        assert!(diagnostic.notes.len() >= 3);
    }

    #[test]
    fn non_spock_files_and_missing_directories_are_not_guessed() {
        let temp = tempdir().unwrap();
        fs::write(temp.path().join("notes.txt"), "notes").unwrap();
        for target in ["notes.txt", "missing"] {
            let diagnostic = resolve_target(Some(Path::new(target)), temp.path())
                .unwrap_err()
                .into_vec()
                .remove(0);
            assert_eq!(diagnostic.code, DiagnosticCode::UnsupportedTarget);
        }
    }

    #[cfg(unix)]
    #[test]
    fn explicit_manifest_symlink_does_not_change_the_selected_root() {
        use std::os::unix::fs::symlink;

        let project = tempdir().unwrap();
        let outside = tempdir().unwrap();
        fs::write(outside.path().join(MANIFEST_FILE), "version = 1\n").unwrap();
        symlink(
            outside.path().join(MANIFEST_FILE),
            project.path().join(MANIFEST_FILE),
        )
        .unwrap();

        let target = resolve_target(Some(Path::new(MANIFEST_FILE)), project.path()).unwrap();
        let ResolvedTarget::Project(root) = target else {
            panic!("explicit manifest did not select project mode");
        };
        assert_eq!(root.path(), fs::canonicalize(project.path()).unwrap());
    }
}

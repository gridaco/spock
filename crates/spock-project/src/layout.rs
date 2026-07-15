use std::fs;
use std::path::{Path, PathBuf};

use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, ProjectResult};
use crate::discovery::{discover_project_root, ProjectRoot};
use crate::manifest::{parse_manifest_file, ProjectManifest, MANIFEST_FILE};
use crate::path::{resolve_contained, ContainedPath, NormalizedRelativePath};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientLayout {
    pub root: ContainedPath,
    pub manifest: ContainedPath,
}

/// Validated filesystem topology for one project. This contains paths and the
/// framework manifest only; language hosts still capture and interpret their
/// own semantic inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectLayout {
    pub root: PathBuf,
    pub manifest_path: PathBuf,
    pub manifest: ProjectManifest,
    pub backend_root: ContainedPath,
    pub backend_entry: ContainedPath,
    pub client: Option<ClientLayout>,
}

pub fn load_project(root: &ProjectRoot) -> ProjectResult<ProjectLayout> {
    let manifest_path = root.manifest_path();
    let manifest_metadata = fs::symlink_metadata(&manifest_path).map_err(|error| {
        Diagnostics::one(
            Diagnostic::new(
                DiagnosticCode::MissingInput,
                format!("could not inspect `{MANIFEST_FILE}`: {error}"),
            )
            .at_path(&manifest_path),
        )
    })?;
    if manifest_metadata.file_type().is_symlink() {
        return Err(Diagnostic::new(
            DiagnosticCode::UnsafeSymlink,
            "the project manifest must be a regular file, not a symlink",
        )
        .at_path(manifest_path)
        .into());
    }
    if !manifest_metadata.is_file() {
        return Err(Diagnostic::new(
            DiagnosticCode::WrongEntryKind,
            "the project manifest is not a regular file",
        )
        .at_path(manifest_path)
        .into());
    }

    let source = fs::read_to_string(&manifest_path).map_err(|error| {
        Diagnostics::one(
            Diagnostic::new(
                DiagnosticCode::Io,
                format!("could not read project manifest: {error}"),
            )
            .at_path(&manifest_path),
        )
    })?;
    let manifest = parse_manifest_file(&source, &manifest_path)?;

    let backend_root = resolve_contained(root.path(), manifest.backend().root())?;
    let backend_entry_relative = manifest.backend().root().join(manifest.backend().entry());
    let backend_entry = resolve_contained(root.path(), &backend_entry_relative)?;

    let mut diagnostics = Diagnostics::new();
    expect_directory(
        backend_root.absolute(),
        "configured backend root",
        &mut diagnostics,
    );
    expect_file(
        backend_entry.absolute(),
        "configured backend entry",
        &mut diagnostics,
    );

    let client = if let Some(config) = manifest.client() {
        let client_root = resolve_contained(root.path(), config.root())?;
        let uhura_name = NormalizedRelativePath::file("uhura.toml")
            .expect("constant Uhura manifest path is valid");
        let client_manifest_relative = config.root().join(&uhura_name);
        let client_manifest = resolve_contained(root.path(), &client_manifest_relative)?;
        expect_directory(
            client_root.absolute(),
            "configured client root",
            &mut diagnostics,
        );
        expect_file(
            client_manifest.absolute(),
            "configured client `uhura.toml`",
            &mut diagnostics,
        );
        Some(ClientLayout {
            root: client_root,
            manifest: client_manifest,
        })
    } else {
        None
    };

    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    Ok(ProjectLayout {
        root: root.path().to_path_buf(),
        manifest_path,
        manifest,
        backend_root,
        backend_entry,
        client,
    })
}

pub fn load_project_from(start: &Path) -> ProjectResult<ProjectLayout> {
    let root = discover_project_root(start)?;
    load_project(&root)
}

fn expect_directory(path: &Path, label: &str, diagnostics: &mut Diagnostics) {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_dir() => {}
        Ok(_) => diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::WrongEntryKind,
                format!("{label} is not a directory"),
            )
            .at_path(path),
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::MissingInput,
                format!("{label} does not exist"),
            )
            .at_path(path),
        ),
        Err(error) => diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::Io,
                format!("could not inspect {label}: {error}"),
            )
            .at_path(path),
        ),
    }
}

fn expect_file(path: &Path, label: &str, diagnostics: &mut Diagnostics) {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::WrongEntryKind,
                format!("{label} is not a regular file"),
            )
            .at_path(path),
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::MissingInput,
                format!("{label} does not exist"),
            )
            .at_path(path),
        ),
        Err(error) => diagnostics.push(
            Diagnostic::new(
                DiagnosticCode::Io,
                format!("could not inspect {label}: {error}"),
            )
            .at_path(path),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn write_manifest(root: &Path, client: bool) {
        let manifest =
            ProjectManifest::new("demo", "backend", "app.spock", client.then_some("client"))
                .unwrap();
        fs::write(root.join(MANIFEST_FILE), manifest.to_toml_string()).unwrap();
    }

    #[test]
    fn loads_validated_backend_and_optional_client_topology() {
        let temp = tempdir().unwrap();
        fs::create_dir(temp.path().join("backend")).unwrap();
        fs::write(temp.path().join("backend/app.spock"), "").unwrap();
        fs::create_dir(temp.path().join("client")).unwrap();
        fs::write(temp.path().join("client/uhura.toml"), "").unwrap();
        write_manifest(temp.path(), true);

        let layout = load_project_from(temp.path()).unwrap();
        assert_eq!(layout.manifest.project().as_str(), "demo");
        assert_eq!(
            layout.backend_entry.absolute(),
            &fs::canonicalize(temp.path().join("backend/app.spock")).unwrap()
        );
        assert!(layout.client.is_some());
    }

    #[test]
    fn missing_backend_and_client_inputs_are_reported_together() {
        let temp = tempdir().unwrap();
        fs::create_dir(temp.path().join("backend")).unwrap();
        fs::create_dir(temp.path().join("client")).unwrap();
        write_manifest(temp.path(), true);

        let diagnostics = load_project_from(temp.path()).unwrap_err();
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == DiagnosticCode::MissingInput));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_root_that_escapes_the_project() {
        use std::os::unix::fs::symlink;

        let project = tempdir().unwrap();
        let outside = tempdir().unwrap();
        fs::write(outside.path().join("app.spock"), "").unwrap();
        symlink(outside.path(), project.path().join("backend")).unwrap();
        write_manifest(project.path(), false);

        let diagnostic = load_project_from(project.path())
            .unwrap_err()
            .into_vec()
            .remove(0);
        assert_eq!(diagnostic.code, DiagnosticCode::PathEscape);
    }

    #[cfg(unix)]
    #[test]
    fn permits_symlinks_whose_canonical_target_stays_inside() {
        use std::os::unix::fs::symlink;

        let project = tempdir().unwrap();
        fs::create_dir(project.path().join("real-backend")).unwrap();
        fs::write(project.path().join("real-backend/app.spock"), "").unwrap();
        symlink("real-backend", project.path().join("backend")).unwrap();
        write_manifest(project.path(), false);

        let layout = load_project_from(project.path()).unwrap();
        assert_eq!(
            layout.backend_root.absolute(),
            &fs::canonicalize(project.path().join("real-backend")).unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_project_manifest() {
        use std::os::unix::fs::symlink;

        let project = tempdir().unwrap();
        let outside = tempdir().unwrap();
        let manifest = ProjectManifest::new("demo", "backend", "app.spock", None)
            .unwrap()
            .to_toml_string();
        fs::write(outside.path().join(MANIFEST_FILE), manifest).unwrap();
        symlink(
            outside.path().join(MANIFEST_FILE),
            project.path().join(MANIFEST_FILE),
        )
        .unwrap();

        let diagnostic = load_project_from(project.path())
            .unwrap_err()
            .into_vec()
            .remove(0);
        assert_eq!(diagnostic.code, DiagnosticCode::UnsafeSymlink);
    }
}

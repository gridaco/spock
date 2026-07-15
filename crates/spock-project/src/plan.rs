use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use unicode_normalization::UnicodeNormalization;

use crate::diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, ProjectResult};
use crate::manifest::{ProjectManifest, MANIFEST_FILE};
use crate::path::NormalizedRelativePath;

pub const DEFAULT_BACKEND_SOURCE: &str =
    "// This project has no authority contract yet. Keep this file empty until it does.\n";

const IGNORED_SCAN_DIRECTORIES: &[&str] = &[".git", ".spock", "node_modules", "target"];

/// Whether a directory is operational noise rather than an adoption input.
///
/// Filesystem adapters outside this crate use the same policy when they walk
/// from an already-pinned directory handle.
pub fn is_ignored_inventory_directory(name: &str) -> bool {
    IGNORED_SCAN_DIRECTORIES.contains(&name)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InventoryEntryKind {
    File,
    Directory,
    Symlink,
    /// A filesystem entry that is neither a regular file, directory, nor
    /// symlink (for example, a Unix socket or FIFO).
    Unsupported,
}

/// A deterministic, read-only view used by pure creation/adoption planning.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProjectInventory {
    root: PathBuf,
    entries: BTreeMap<NormalizedRelativePath, InventoryEntryKind>,
}

impl ProjectInventory {
    pub fn empty(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            entries: BTreeMap::new(),
        }
    }

    pub fn from_entries(
        root: impl Into<PathBuf>,
        entries: impl IntoIterator<Item = (NormalizedRelativePath, InventoryEntryKind)>,
    ) -> ProjectResult<Self> {
        let mut collected = BTreeMap::new();
        let mut portable_names = BTreeMap::<String, NormalizedRelativePath>::new();
        let mut diagnostics = Diagnostics::new();
        for (path, kind) in entries {
            let key = portable_case_key(&path);
            if let Some(existing) = portable_names.get(&key) {
                let message = if existing == &path {
                    format!("inventory contains duplicate path `{path}`")
                } else {
                    format!(
                        "inventory contains `{existing}` and `{path}`, which name the same destination on supported case- or normalization-insensitive filesystems"
                    )
                };
                diagnostics.push(Diagnostic::new(DiagnosticCode::PlanConflict, message));
                continue;
            }
            portable_names.insert(key, path.clone());
            collected.insert(path, kind);
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        Ok(Self {
            root: root.into(),
            entries: collected,
        })
    }

    /// Capture names and entry kinds only. File contents remain owned by the
    /// language-specific capture layers.
    pub fn scan(root: &Path) -> ProjectResult<Self> {
        let canonical_root = fs::canonicalize(root).map_err(|error| {
            Diagnostics::one(
                Diagnostic::new(
                    DiagnosticCode::Io,
                    format!("could not resolve adoption root: {error}"),
                )
                .at_path(root),
            )
        })?;
        if !canonical_root.is_dir() {
            return Err(Diagnostic::new(
                DiagnosticCode::WrongEntryKind,
                "adoption root is not a directory",
            )
            .at_path(root)
            .into());
        }

        let mut entries = BTreeMap::new();
        scan_directory(&canonical_root, &canonical_root, &mut entries)?;
        Self::from_entries(canonical_root, entries)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn entries(
        &self,
    ) -> impl ExactSizeIterator<Item = (&NormalizedRelativePath, InventoryEntryKind)> {
        self.entries.iter().map(|(path, kind)| (path, *kind))
    }

    pub fn kind(&self, path: &NormalizedRelativePath) -> Option<InventoryEntryKind> {
        self.entries.get(path).copied()
    }
}

fn scan_directory(
    root: &Path,
    directory: &Path,
    entries: &mut BTreeMap<NormalizedRelativePath, InventoryEntryKind>,
) -> ProjectResult<()> {
    let mut children = fs::read_dir(directory)
        .map_err(|error| {
            Diagnostics::one(
                Diagnostic::new(
                    DiagnosticCode::Io,
                    format!("could not scan directory: {error}"),
                )
                .at_path(directory),
            )
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| {
            Diagnostics::one(
                Diagnostic::new(
                    DiagnosticCode::Io,
                    format!("could not scan directory entry: {error}"),
                )
                .at_path(directory),
            )
        })?;
    children.sort_by_key(fs::DirEntry::file_name);

    for child in children {
        let path = child.path();
        let relative = portable_relative(root, &path)?;
        let file_type = child.file_type().map_err(|error| {
            Diagnostics::one(
                Diagnostic::new(
                    DiagnosticCode::Io,
                    format!("could not inspect directory entry: {error}"),
                )
                .at_path(&path),
            )
        })?;
        let kind = if file_type.is_symlink() {
            InventoryEntryKind::Symlink
        } else if file_type.is_dir() {
            InventoryEntryKind::Directory
        } else if file_type.is_file() {
            InventoryEntryKind::File
        } else {
            InventoryEntryKind::Unsupported
        };
        entries.insert(relative.clone(), kind);

        if kind == InventoryEntryKind::Directory
            && !relative
                .file_name()
                .is_some_and(is_ignored_inventory_directory)
        {
            scan_directory(root, &path, entries)?;
        }
    }
    Ok(())
}

fn portable_relative(root: &Path, path: &Path) -> ProjectResult<NormalizedRelativePath> {
    let relative = path.strip_prefix(root).map_err(|_| {
        Diagnostics::one(
            Diagnostic::new(
                DiagnosticCode::PathEscape,
                "scanned path escaped the inventory root",
            )
            .at_path(path),
        )
    })?;
    let mut segments = Vec::new();
    for component in relative.components() {
        let std::path::Component::Normal(segment) = component else {
            return Err(Diagnostic::new(
                DiagnosticCode::InvalidManifestPath,
                "scanned path cannot be represented in a project manifest",
            )
            .at_path(path)
            .into());
        };
        let Some(segment) = segment.to_str() else {
            return Err(Diagnostic::new(
                DiagnosticCode::InvalidManifestPath,
                "scanned path is not UTF-8 and cannot be represented in a project manifest",
            )
            .at_path(path)
            .into());
        };
        segments.push(segment);
    }
    let portable = segments.join("/");
    NormalizedRelativePath::file(&portable).map_err(|error| {
        Diagnostics::one(
            Diagnostic::new(
                DiagnosticCode::InvalidManifestPath,
                format!("scanned path is not a valid project path: {error}"),
            )
            .at_path(path),
        )
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateFile {
    path: NormalizedRelativePath,
    contents: Vec<u8>,
}

impl TemplateFile {
    pub fn new(path: &str, contents: impl Into<Vec<u8>>) -> ProjectResult<Self> {
        let path = NormalizedRelativePath::file(path).map_err(|error| {
            Diagnostics::one(Diagnostic::new(
                DiagnosticCode::InvalidTemplate,
                format!("invalid template path `{path}`: {error}"),
            ))
        })?;
        Ok(Self {
            path,
            contents: contents.into(),
        })
    }

    pub fn path(&self) -> &NormalizedRelativePath {
        &self.path
    }

    pub fn contents(&self) -> &[u8] {
        &self.contents
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClientTemplate {
    files: Vec<TemplateFile>,
}

impl ClientTemplate {
    pub fn new(mut files: Vec<TemplateFile>) -> ProjectResult<Self> {
        files.sort_by(|left, right| left.path.cmp(&right.path));
        let mut diagnostics = Diagnostics::new();
        for pair in files.windows(2) {
            if pair[0].path == pair[1].path {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::InvalidTemplate,
                    format!("client template repeats `{}`", pair[0].path),
                ));
            }
        }
        if !files.iter().any(|file| file.path.as_str() == "uhura.toml") {
            diagnostics.push(Diagnostic::new(
                DiagnosticCode::InvalidTemplate,
                "client template must contain `uhura.toml` at its root",
            ));
        }
        if !diagnostics.is_empty() {
            return Err(diagnostics);
        }
        Ok(Self { files })
    }

    pub fn files(&self) -> &[TemplateFile] {
        &self.files
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlanKind {
    Scaffold,
    Adopt,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlannedWrite {
    pub relative_path: NormalizedRelativePath,
    pub contents: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WritePlan {
    pub kind: PlanKind,
    pub root: PathBuf,
    writes: Vec<PlannedWrite>,
}

impl WritePlan {
    pub fn writes(&self) -> &[PlannedWrite] {
        &self.writes
    }

    pub fn write(&self, path: &str) -> Option<&PlannedWrite> {
        self.writes
            .iter()
            .find(|write| write.relative_path.as_str() == path)
    }

    /// Check all conflicts without mutating the inventory or filesystem.
    pub fn preflight(&self, inventory: &ProjectInventory) -> ProjectResult<()> {
        if self.root != inventory.root {
            return Err(Diagnostic::new(
                DiagnosticCode::PlanConflict,
                format!(
                    "plan root {} does not match inventory root {}",
                    self.root.display(),
                    inventory.root.display()
                ),
            )
            .into());
        }

        let mut diagnostics = Diagnostics::new();
        let portable_entries = inventory
            .entries
            .iter()
            .map(|(path, kind)| (portable_case_key(path), (path, *kind)))
            .collect::<BTreeMap<_, _>>();
        for write in &self.writes {
            if let Some((existing, kind)) =
                portable_entries.get(&portable_case_key(&write.relative_path))
            {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::PlanConflict,
                    format!(
                        "would overwrite existing {} `{existing}` with planned path `{}`",
                        kind_name(*kind),
                        write.relative_path
                    ),
                ));
                continue;
            }
            let mut parent = write.relative_path.parent();
            while !parent.is_project_root() {
                if let Some((existing, kind)) = portable_entries.get(&portable_case_key(&parent)) {
                    if *existing != &parent || *kind != InventoryEntryKind::Directory {
                        let reason = if *existing != &parent {
                            format!(
                                "ancestor `{parent}` aliases existing {} `{existing}` on supported case- or normalization-insensitive filesystems",
                                kind_name(*kind)
                            )
                        } else {
                            format!("ancestor `{parent}` is an existing {}", kind_name(*kind))
                        };
                        diagnostics.push(Diagnostic::new(
                            DiagnosticCode::PlanConflict,
                            format!("cannot create `{}` because {reason}", write.relative_path),
                        ));
                        break;
                    }
                }
                parent = parent.parent();
            }
        }
        if diagnostics.is_empty() {
            Ok(())
        } else {
            Err(diagnostics)
        }
    }
}

fn kind_name(kind: InventoryEntryKind) -> &'static str {
    match kind {
        InventoryEntryKind::File => "file",
        InventoryEntryKind::Directory => "directory",
        InventoryEntryKind::Symlink => "symlink",
        InventoryEntryKind::Unsupported => "unsupported filesystem entry",
    }
}

/// Produce the canonical new-project writes without touching the destination.
pub fn scaffold_plan(
    destination: impl Into<PathBuf>,
    project_name: &str,
    client: Option<&ClientTemplate>,
) -> ProjectResult<WritePlan> {
    let destination = destination.into();
    if destination.as_os_str().is_empty() {
        return Err(Diagnostic::new(
            DiagnosticCode::PlanConflict,
            "scaffold destination must not be empty",
        )
        .into());
    }
    let manifest = ProjectManifest::new(
        project_name,
        "backend",
        "app.spock",
        client.map(|_| "client"),
    )?;
    let mut writes = vec![
        planned(MANIFEST_FILE, manifest.to_toml_string().into_bytes())?,
        planned(
            "backend/app.spock",
            DEFAULT_BACKEND_SOURCE.as_bytes().to_vec(),
        )?,
    ];
    if let Some(client) = client {
        let client_root =
            NormalizedRelativePath::root("client").expect("constant client root is valid");
        for file in client.files() {
            writes.push(PlannedWrite {
                relative_path: client_root.join(file.path()),
                contents: file.contents.clone(),
            });
        }
    }
    finish_plan(PlanKind::Scaffold, destination, writes)
}

/// Plan adoption from names and entry kinds only. Existing sources are never
/// rewritten or moved.
pub fn adoption_plan(
    inventory: &ProjectInventory,
    project_name: Option<&str>,
) -> ProjectResult<WritePlan> {
    let manifest_path = NormalizedRelativePath::file(MANIFEST_FILE)
        .expect("constant framework manifest path is valid");
    if inventory.kind(&manifest_path).is_some() {
        return Err(Diagnostic::new(
            DiagnosticCode::AlreadyProject,
            format!("`{MANIFEST_FILE}` already exists; this directory is already adopted"),
        )
        .at_path(inventory.root.join(MANIFEST_FILE))
        .into());
    }

    let backend_candidates = candidates(inventory, |path| path.extension() == Some("spock"));
    let client_candidates = candidates(inventory, |path| path.file_name() == Some("uhura.toml"));
    reject_ambiguous_or_symlinked(
        &backend_candidates,
        DiagnosticCode::AmbiguousBackend,
        "Spock backend",
    )?;
    reject_ambiguous_or_symlinked(
        &client_candidates,
        DiagnosticCode::AmbiguousClient,
        "Uhura client",
    )?;

    let name = match project_name {
        Some(name) => name.to_string(),
        None => inventory
            .root
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                Diagnostics::one(Diagnostic::new(
                    DiagnosticCode::InvalidProjectName,
                    "could not derive a UTF-8 project name from the adoption root",
                ))
            })?
            .to_string(),
    };

    let (backend_root, backend_entry, create_backend) = match backend_candidates.first() {
        Some((path, InventoryEntryKind::File)) => (
            path.parent(),
            NormalizedRelativePath::file(path.file_name().expect("candidate is a file path"))
                .expect("candidate filename remains normalized"),
            false,
        ),
        None => (
            NormalizedRelativePath::root("backend").expect("constant root is valid"),
            NormalizedRelativePath::file("app.spock").expect("constant entry is valid"),
            true,
        ),
        Some(_) => unreachable!("symlink candidates rejected above"),
    };
    let client_root = client_candidates.first().map(|(path, _)| path.parent());
    let manifest = ProjectManifest::new(
        &name,
        backend_root.as_str(),
        backend_entry.as_str(),
        client_root.as_ref().map(NormalizedRelativePath::as_str),
    )?;

    let mut writes = vec![planned(
        MANIFEST_FILE,
        manifest.to_toml_string().into_bytes(),
    )?];
    if create_backend {
        writes.push(planned(
            "backend/app.spock",
            DEFAULT_BACKEND_SOURCE.as_bytes().to_vec(),
        )?);
    }
    let plan = finish_plan(PlanKind::Adopt, inventory.root.clone(), writes)?;
    plan.preflight(inventory)?;
    Ok(plan)
}

fn candidates<F>(
    inventory: &ProjectInventory,
    predicate: F,
) -> Vec<(NormalizedRelativePath, InventoryEntryKind)>
where
    F: Fn(&NormalizedRelativePath) -> bool,
{
    inventory
        .entries()
        .filter(|(path, kind)| {
            matches!(kind, InventoryEntryKind::File | InventoryEntryKind::Symlink)
                && predicate(path)
        })
        .map(|(path, kind)| (path.clone(), kind))
        .collect()
}

fn reject_ambiguous_or_symlinked(
    candidates: &[(NormalizedRelativePath, InventoryEntryKind)],
    ambiguity_code: DiagnosticCode,
    label: &str,
) -> ProjectResult<()> {
    if candidates.len() > 1 {
        let mut diagnostic = Diagnostic::new(
            ambiguity_code,
            format!("found multiple {label} candidates; choose one explicitly"),
        );
        for (path, _) in candidates {
            diagnostic = diagnostic.with_note(path.to_string());
        }
        return Err(diagnostic.into());
    }
    if let Some((path, InventoryEntryKind::Symlink)) = candidates.first() {
        return Err(Diagnostic::new(
            DiagnosticCode::UnsafeSymlink,
            format!("cannot adopt symlinked {label} candidate `{path}`"),
        )
        .into());
    }
    Ok(())
}

fn planned(path: &str, contents: Vec<u8>) -> ProjectResult<PlannedWrite> {
    let relative_path = NormalizedRelativePath::file(path).map_err(|error| {
        Diagnostics::one(Diagnostic::new(
            DiagnosticCode::InvalidTemplate,
            format!("invalid planned path `{path}`: {error}"),
        ))
    })?;
    Ok(PlannedWrite {
        relative_path,
        contents,
    })
}

fn finish_plan(
    kind: PlanKind,
    root: PathBuf,
    mut writes: Vec<PlannedWrite>,
) -> ProjectResult<WritePlan> {
    writes.sort_by(|left, right| left.relative_path.cmp(&right.relative_path));
    let mut seen = BTreeMap::<String, NormalizedRelativePath>::new();
    let mut diagnostics = Diagnostics::new();
    for write in &writes {
        let key = portable_case_key(&write.relative_path);
        if let Some(existing) = seen.get(&key) {
            let message = if existing == &write.relative_path {
                format!("plan writes `{}` more than once", write.relative_path)
            } else {
                format!(
                    "plan writes `{existing}` and `{}`, which name the same destination on supported case- or normalization-insensitive filesystems",
                    write.relative_path
                )
            };
            diagnostics.push(Diagnostic::new(DiagnosticCode::PlanConflict, message));
        } else {
            seen.insert(key, write.relative_path.clone());
        }
    }

    // Equality is not the only impossible file topology. A plan that writes
    // both `foo` and `foo/bar` would partially mutate the destination before
    // apply discovers that `foo` cannot be both a file and a directory. Check
    // every portable parent key in a second pass so case or normalization
    // aliases are caught even when lexical sorting places the descendant first.
    let planned_paths = writes
        .iter()
        .map(|write| {
            (
                portable_case_key(&write.relative_path),
                &write.relative_path,
            )
        })
        .collect::<BTreeMap<_, _>>();
    for write in &writes {
        let mut parent = write.relative_path.parent();
        while !parent.is_project_root() {
            if let Some(existing) = planned_paths.get(&portable_case_key(&parent)) {
                diagnostics.push(Diagnostic::new(
                    DiagnosticCode::PlanConflict,
                    format!(
                        "cannot create `{}` because planned file `{existing}` is its ancestor",
                        write.relative_path
                    ),
                ));
                break;
            }
            parent = parent.parent();
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(WritePlan { kind, root, writes })
}

fn portable_case_key(path: &NormalizedRelativePath) -> String {
    // Windows' ordinal case-insensitive comparison is based on uppercase
    // mappings. Canonical decomposition also collapses the normalization
    // aliases used by supported macOS filesystems. Applying both is
    // deliberately conservative for portable planning (for example, both
    // Greek sigma spellings map to Σ, and é aliases e + combining acute).
    path.as_str()
        .chars()
        .flat_map(char::to_uppercase)
        .nfd()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn entry(path: &str, kind: InventoryEntryKind) -> (NormalizedRelativePath, InventoryEntryKind) {
        (NormalizedRelativePath::file(path).unwrap(), kind)
    }

    #[test]
    fn scaffold_is_deterministic_and_mutation_free() {
        let destination = PathBuf::from("/future/demo");
        let plan = scaffold_plan(&destination, "demo", None).unwrap();
        assert_eq!(plan.root, destination);
        assert_eq!(
            plan.writes()
                .iter()
                .map(|write| write.relative_path.as_str())
                .collect::<Vec<_>>(),
            ["backend/app.spock", "spock.toml"]
        );
        let manifest =
            std::str::from_utf8(plan.write("spock.toml").unwrap().contents.as_slice()).unwrap();
        assert!(manifest.contains("root = \"backend\""));
        assert!(!destination.exists());
    }

    #[test]
    fn client_template_is_opaque_but_requires_its_manifest() {
        let missing = ClientTemplate::new(vec![TemplateFile::new("app/main.uhura", "").unwrap()]);
        assert_eq!(
            missing.unwrap_err().into_vec()[0].code,
            DiagnosticCode::InvalidTemplate
        );

        let template = ClientTemplate::new(vec![
            TemplateFile::new("uhura.toml", "[app]\nname = \"demo\"\n").unwrap(),
            TemplateFile::new("app/main.uhura", "screen main {}\n").unwrap(),
        ])
        .unwrap();
        let plan = scaffold_plan("demo", "demo", Some(&template)).unwrap();
        assert!(plan.write("client/uhura.toml").is_some());
        assert!(plan.write("client/app/main.uhura").is_some());
        let manifest =
            std::str::from_utf8(plan.write("spock.toml").unwrap().contents.as_slice()).unwrap();
        assert!(manifest.contains("[client]"));
    }

    #[test]
    fn write_plan_rejects_case_insensitive_destination_aliases() {
        let diagnostics = finish_plan(
            PlanKind::Scaffold,
            PathBuf::from("/future/demo"),
            vec![
                planned("client/App/page.uhura", Vec::new()).unwrap(),
                planned("client/app/PAGE.uhura", Vec::new()).unwrap(),
            ],
        )
        .unwrap_err();

        assert_eq!(diagnostics.len(), 1);
        let diagnostic = diagnostics.into_vec().remove(0);
        assert_eq!(diagnostic.code, DiagnosticCode::PlanConflict);
        assert!(diagnostic.message.contains("client/App/page.uhura"));
        assert!(diagnostic.message.contains("client/app/PAGE.uhura"));
        assert!(diagnostic
            .message
            .contains("case- or normalization-insensitive filesystems"));
    }

    #[test]
    fn write_plan_rejects_file_ancestor_aliases_before_apply() {
        for (ancestor, descendant) in [
            ("client/foo", "client/foo/bar.uhura"),
            // The descendant sorts before the ancestor, proving validation is
            // independent of the writes' lexical order.
            ("client/z", "client/Z/bar.uhura"),
            ("client/café", "client/cafe\u{301}/bar.uhura"),
        ] {
            let diagnostics = finish_plan(
                PlanKind::Scaffold,
                PathBuf::from("/future/demo"),
                vec![
                    planned(ancestor, Vec::new()).unwrap(),
                    planned(descendant, Vec::new()).unwrap(),
                ],
            )
            .unwrap_err();

            assert_eq!(diagnostics.len(), 1, "{ancestor} versus {descendant}");
            let diagnostic = &diagnostics.iter().next().unwrap();
            assert_eq!(diagnostic.code, DiagnosticCode::PlanConflict);
            assert!(diagnostic.message.contains(ancestor));
            assert!(diagnostic.message.contains(descendant));
            assert!(diagnostic.message.contains("ancestor"));
        }

        let siblings = finish_plan(
            PlanKind::Scaffold,
            PathBuf::from("/future/demo"),
            vec![
                planned("client/foo", Vec::new()).unwrap(),
                planned("client/foobar/page.uhura", Vec::new()).unwrap(),
            ],
        )
        .unwrap();
        assert_eq!(siblings.writes().len(), 2);
    }

    #[test]
    fn portable_alias_checks_cover_inventory_and_unicode_uppercase_equivalence() {
        let inventory_error = ProjectInventory::from_entries(
            "/project",
            [
                entry("client/App/page.uhura", InventoryEntryKind::File),
                entry("client/app/PAGE.uhura", InventoryEntryKind::File),
            ],
        )
        .unwrap_err();
        assert_eq!(inventory_error.len(), 1);
        assert!(inventory_error.into_vec()[0]
            .message
            .contains("case- or normalization-insensitive filesystems"));

        let unicode_error = finish_plan(
            PlanKind::Scaffold,
            PathBuf::from("/future/demo"),
            vec![
                planned("client/σ.uhura", Vec::new()).unwrap(),
                planned("client/ς.uhura", Vec::new()).unwrap(),
            ],
        )
        .unwrap_err();
        assert_eq!(unicode_error.len(), 1);

        let normalization_error = finish_plan(
            PlanKind::Scaffold,
            PathBuf::from("/future/demo"),
            vec![
                planned("client/café.uhura", Vec::new()).unwrap(),
                planned("client/cafe\u{301}.uhura", Vec::new()).unwrap(),
            ],
        )
        .unwrap_err();
        assert_eq!(normalization_error.len(), 1);
    }

    #[test]
    fn preflight_rejects_existing_case_aliases_before_any_write() {
        let root = PathBuf::from("/project");
        let inventory = ProjectInventory::from_entries(
            &root,
            [
                entry("SPOCK.TOML", InventoryEntryKind::File),
                entry("Backend", InventoryEntryKind::Directory),
            ],
        )
        .unwrap();
        let plan = scaffold_plan(&root, "demo", None).unwrap();

        let diagnostics = plan.preflight(&inventory).unwrap_err();

        assert_eq!(diagnostics.len(), 2);
        let messages = diagnostics
            .iter()
            .map(|diagnostic| diagnostic.message.as_str())
            .collect::<Vec<_>>();
        assert!(messages
            .iter()
            .any(|message| message.contains("SPOCK.TOML")));
        assert!(messages.iter().any(|message| message.contains("Backend")));
    }

    #[test]
    fn preflight_reports_every_overwrite_and_blocking_ancestor() {
        let root = PathBuf::from("/project");
        let inventory = ProjectInventory::from_entries(
            &root,
            [
                entry("spock.toml", InventoryEntryKind::File),
                entry("backend", InventoryEntryKind::File),
            ],
        )
        .unwrap();
        let plan = scaffold_plan(&root, "demo", None).unwrap();
        let diagnostics = plan.preflight(&inventory).unwrap_err();
        assert_eq!(diagnostics.len(), 2);
        assert!(diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == DiagnosticCode::PlanConflict));
    }

    #[test]
    fn adoption_references_existing_sources_without_writing_them() {
        let root = PathBuf::from("/project");
        let inventory = ProjectInventory::from_entries(
            &root,
            [
                entry("server/main.spock", InventoryEntryKind::File),
                entry("experience/uhura.toml", InventoryEntryKind::File),
            ],
        )
        .unwrap();
        let plan = adoption_plan(&inventory, Some("adopted")).unwrap();
        assert_eq!(plan.writes().len(), 1);
        let source =
            std::str::from_utf8(plan.write(MANIFEST_FILE).unwrap().contents.as_slice()).unwrap();
        let manifest = crate::manifest::parse_manifest(source).unwrap();
        assert_eq!(manifest.backend().root().as_str(), "server");
        assert_eq!(manifest.backend().entry().as_str(), "main.spock");
        assert_eq!(manifest.client().unwrap().root().as_str(), "experience");
    }

    #[test]
    fn uhura_only_adoption_adds_an_explicit_empty_backend() {
        let root = PathBuf::from("/project");
        let inventory =
            ProjectInventory::from_entries(&root, [entry("uhura.toml", InventoryEntryKind::File)])
                .unwrap();
        let plan = adoption_plan(&inventory, Some("client-first")).unwrap();
        assert!(plan.write("backend/app.spock").is_some());
        let source =
            std::str::from_utf8(plan.write(MANIFEST_FILE).unwrap().contents.as_slice()).unwrap();
        let manifest = crate::manifest::parse_manifest(source).unwrap();
        assert_eq!(manifest.client().unwrap().root().as_str(), ".");
    }

    #[test]
    fn empty_directory_adoption_creates_only_manifest_and_empty_authority() {
        let inventory = ProjectInventory::empty("/project");
        let plan = adoption_plan(&inventory, Some("empty")).unwrap();
        assert_eq!(
            plan.writes()
                .iter()
                .map(|write| write.relative_path.as_str())
                .collect::<Vec<_>>(),
            ["backend/app.spock", "spock.toml"]
        );
        let source =
            std::str::from_utf8(plan.write(MANIFEST_FILE).unwrap().contents.as_slice()).unwrap();
        let manifest = crate::manifest::parse_manifest(source).unwrap();
        assert!(manifest.client().is_none());
    }

    #[test]
    fn existing_framework_manifest_is_never_overwritten_by_adoption() {
        let inventory = ProjectInventory::from_entries(
            "/project",
            [entry(MANIFEST_FILE, InventoryEntryKind::File)],
        )
        .unwrap();
        let diagnostic = adoption_plan(&inventory, Some("demo"))
            .unwrap_err()
            .into_vec()
            .remove(0);
        assert_eq!(diagnostic.code, DiagnosticCode::AlreadyProject);
    }

    #[test]
    fn ambiguous_adoption_lists_sorted_choices() {
        let inventory = ProjectInventory::from_entries(
            "/project",
            [
                entry("z.spock", InventoryEntryKind::File),
                entry("a.spock", InventoryEntryKind::File),
            ],
        )
        .unwrap();
        let diagnostic = adoption_plan(&inventory, Some("demo"))
            .unwrap_err()
            .into_vec()
            .remove(0);
        assert_eq!(diagnostic.code, DiagnosticCode::AmbiguousBackend);
        assert_eq!(diagnostic.notes, ["a.spock", "z.spock"]);
    }

    #[test]
    fn scan_is_sorted_does_not_follow_noise_and_keeps_declared_dist_files() {
        let temp = tempdir().unwrap();
        fs::create_dir_all(temp.path().join("target/nested")).unwrap();
        fs::write(temp.path().join("target/nested/ignored.spock"), "").unwrap();
        fs::create_dir_all(temp.path().join("providers/dist")).unwrap();
        fs::write(temp.path().join("providers/dist/spock.js"), "").unwrap();
        fs::write(temp.path().join("app.spock"), "").unwrap();

        let inventory = ProjectInventory::scan(temp.path()).unwrap();
        let paths = inventory
            .entries()
            .map(|(path, _)| path.as_str())
            .collect::<Vec<_>>();
        assert!(paths.contains(&"providers/dist/spock.js"));
        assert!(!paths.contains(&"target/nested/ignored.spock"));
        assert!(paths.windows(2).all(|pair| pair[0] <= pair[1]));
    }

    #[cfg(unix)]
    #[test]
    fn scan_represents_special_entries_without_adopting_them_as_sources() {
        use std::os::unix::net::UnixListener;

        let temp = tempdir().unwrap();
        let socket_path = temp.path().join("app.spock");
        let _socket = UnixListener::bind(&socket_path).unwrap();

        let inventory = ProjectInventory::scan(temp.path()).unwrap();
        let socket = NormalizedRelativePath::file("app.spock").unwrap();
        assert_eq!(
            inventory.kind(&socket),
            Some(InventoryEntryKind::Unsupported)
        );

        let plan = adoption_plan(&inventory, Some("demo")).unwrap();
        assert!(plan.write("backend/app.spock").is_some());
        let manifest =
            std::str::from_utf8(plan.write(MANIFEST_FILE).unwrap().contents.as_slice()).unwrap();
        assert!(manifest.contains("root = \"backend\""));
        assert!(manifest.contains("entry = \"app.spock\""));
    }

    #[cfg(unix)]
    #[test]
    fn adoption_refuses_symlinked_semantic_roots() {
        let inventory = ProjectInventory::from_entries(
            "/project",
            [entry("app.spock", InventoryEntryKind::Symlink)],
        )
        .unwrap();
        let diagnostic = adoption_plan(&inventory, Some("demo"))
            .unwrap_err()
            .into_vec()
            .remove(0);
        assert_eq!(diagnostic.code, DiagnosticCode::UnsafeSymlink);
    }
}

//! Coherent, database-free observation of Spock backend inputs.
//!
//! The observer captures the configured source and every checked
//! `file("...")` seed dependency as one immutable byte bundle. It never opens a
//! runtime generation or touches a database; activation policy belongs to the
//! generation coordinator and process supervisor.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs;
use std::ops::Range;
use std::path::{Path, PathBuf};

use spock_lang::ir::SeedValue;
use spock_project::ProjectLayout;
use spock_runtime::generation::CapturedBackend;

use crate::Fingerprint;

const MAX_STABILITY_SAMPLES: usize = 4;
const INVALID_OBSERVATION_PROTOCOL: &[u8] = b"spock-invalid-backend-observation/1";

/// Stable categories for backend-capture failures.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackendDiagnosticCode {
    Io,
    InvalidUtf8,
    Language,
    PathEscape,
    WrongEntryKind,
    UnstableInputs,
}

impl BackendDiagnosticCode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Io => "SPH001",
            Self::InvalidUtf8 => "SPH002",
            Self::Language => "SPH003",
            Self::PathEscape => "SPH004",
            Self::WrongEntryKind => "SPH005",
            Self::UnstableInputs => "SPH006",
        }
    }
}

impl fmt::Display for BackendDiagnosticCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// One diagnostic from a coherent backend observation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BackendDiagnostic {
    pub code: BackendDiagnosticCode,
    pub message: String,
    pub path: Option<PathBuf>,
    pub span: Option<Range<usize>>,
    /// The Spock language diagnostic code when `code` is `Language`.
    pub language_code: Option<&'static str>,
}

impl BackendDiagnostic {
    fn new(code: BackendDiagnosticCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            path: None,
            span: None,
            language_code: None,
        }
    }

    fn at_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }

    fn stable_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        push_field(&mut bytes, self.code.as_str().as_bytes());
        push_field(
            &mut bytes,
            self.path
                .as_deref()
                .unwrap_or_else(|| Path::new(""))
                .as_os_str()
                .to_string_lossy()
                .as_bytes(),
        );
        push_field(
            &mut bytes,
            self.language_code.unwrap_or_default().as_bytes(),
        );
        if let Some(span) = &self.span {
            push_field(&mut bytes, &span.start.to_be_bytes());
            push_field(&mut bytes, &span.end.to_be_bytes());
        } else {
            push_field(&mut bytes, &[]);
            push_field(&mut bytes, &[]);
        }
        push_field(&mut bytes, self.message.as_bytes());
        bytes
    }
}

impl fmt::Display for BackendDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: ", self.code)?;
        if let Some(path) = &self.path {
            write!(formatter, "{}: ", path.display())?;
        }
        if let Some(language_code) = self.language_code {
            write!(formatter, "error[{language_code}]: ")?;
        }
        formatter.write_str(&self.message)
    }
}

/// Deterministically ordered diagnostics from one backend observation.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct BackendDiagnostics(Vec<BackendDiagnostic>);

impl BackendDiagnostics {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = &BackendDiagnostic> {
        self.0.iter()
    }

    fn push(&mut self, diagnostic: BackendDiagnostic) {
        self.0.push(diagnostic);
    }
}

impl fmt::Display for BackendDiagnostics {
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

impl std::error::Error for BackendDiagnostics {}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObservedInput {
    display_name: String,
    fingerprint: Fingerprint,
}

/// One stable filesystem observation, valid or invalid.
///
/// Invalid source states still have an identity, allowing `spock dev` to
/// report transitions and recovery without constructing a database-backed
/// runtime candidate.
#[derive(Clone, Debug)]
pub struct BackendObservation {
    fingerprint: Fingerprint,
    inputs: BTreeMap<String, ObservedInput>,
    captured: Option<CapturedBackend>,
    diagnostics: BackendDiagnostics,
}

impl BackendObservation {
    #[must_use]
    pub fn fingerprint(&self) -> &Fingerprint {
        &self.fingerprint
    }

    #[must_use]
    pub fn captured_backend(&self) -> Option<&CapturedBackend> {
        self.captured.as_ref()
    }

    #[must_use]
    pub fn diagnostics(&self) -> &BackendDiagnostics {
        &self.diagnostics
    }

    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.captured.is_some()
    }

    /// Human-facing names for inputs whose value or availability differs.
    #[must_use]
    pub fn changed_inputs_since(&self, previous: &Self) -> Vec<String> {
        let identities = self
            .inputs
            .keys()
            .chain(previous.inputs.keys())
            .collect::<BTreeSet<_>>();
        identities
            .into_iter()
            .filter_map(|identity| {
                let current = self.inputs.get(identity);
                let previous = previous.inputs.get(identity);
                (current != previous).then(|| {
                    current
                        .or(previous)
                        .expect("identity came from one input map")
                        .display_name
                        .clone()
                })
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn into_captured_backend(self) -> Result<CapturedBackend, BackendDiagnostics> {
        match self.captured {
            Some(captured) => Ok(captured),
            None => Err(self.diagnostics),
        }
    }
}

/// Observe the backend without constructing, opening, reseeding, or swapping
/// any runtime generation.
#[must_use]
pub fn observe_backend(layout: &ProjectLayout) -> BackendObservation {
    let sample = stable_sample(|| sample_backend(layout));
    BackendObservation::from_sample(sample)
}

/// Capture a valid immutable runtime input bundle.
pub fn capture_backend(layout: &ProjectLayout) -> Result<CapturedBackend, BackendDiagnostics> {
    observe_backend(layout).into_captured_backend()
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum InputContent {
    Bytes(Vec<u8>),
    Unavailable(String),
}

impl InputContent {
    fn stable_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        match self {
            Self::Bytes(value) => {
                bytes.extend_from_slice(b"bytes");
                push_field(&mut bytes, value);
            }
            Self::Unavailable(reason) => {
                bytes.extend_from_slice(b"unavailable");
                push_field(&mut bytes, reason.as_bytes());
            }
        }
        bytes
    }

    fn bytes(&self) -> Option<&[u8]> {
        match self {
            Self::Bytes(bytes) => Some(bytes),
            Self::Unavailable(_) => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SampleInput {
    display_name: String,
    requested_path: PathBuf,
    canonical_path: Option<PathBuf>,
    content: InputContent,
}

impl SampleInput {
    fn unavailable(
        display_name: impl Into<String>,
        requested_path: PathBuf,
        reason: impl Into<String>,
    ) -> Self {
        Self {
            display_name: display_name.into(),
            requested_path,
            canonical_path: None,
            content: InputContent::Unavailable(reason.into()),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct BackendSample {
    source: SampleInput,
    assets: BTreeMap<String, SampleInput>,
    diagnostics: BackendDiagnostics,
}

impl BackendSample {
    fn new(layout: &ProjectLayout, source_path: PathBuf) -> Self {
        Self {
            source: SampleInput::unavailable(
                layout.backend_entry.relative().as_str(),
                source_path,
                "source has not been read",
            ),
            assets: BTreeMap::new(),
            diagnostics: BackendDiagnostics::default(),
        }
    }

    fn source_failure(mut self, code: BackendDiagnosticCode, message: impl Into<String>) -> Self {
        let message = message.into();
        self.source.content = InputContent::Unavailable(message.clone());
        self.diagnostics.push(
            BackendDiagnostic::new(code, message).at_path(self.source.requested_path.clone()),
        );
        self
    }
}

impl BackendObservation {
    fn from_sample(sample: BackendSample) -> Self {
        let valid = sample.diagnostics.is_empty();
        let source_bytes = sample.source.content.bytes().unwrap_or_default();
        let asset_bytes = sample
            .assets
            .iter()
            .filter_map(|(spelling, input)| {
                input
                    .content
                    .bytes()
                    .map(|bytes| (spelling.clone(), bytes.to_vec()))
            })
            .collect::<BTreeMap<_, _>>();

        let captured_candidate = CapturedBackend::new(source_bytes, asset_bytes);
        let fingerprint = if valid {
            Fingerprint::new(captured_candidate.input_fingerprint().as_str())
        } else {
            invalid_observation_fingerprint(&sample)
        };

        let mut inputs = BTreeMap::new();
        inputs.insert(
            format!("source:{}", sample.source.display_name),
            observed_input("source", &sample.source),
        );
        for (spelling, input) in &sample.assets {
            inputs.insert(
                format!("seed:{spelling}"),
                observed_input(&format!("seed:{spelling}"), input),
            );
        }

        Self {
            fingerprint,
            inputs,
            captured: valid.then_some(captured_candidate),
            diagnostics: sample.diagnostics,
        }
    }
}

fn observed_input(identity: &str, input: &SampleInput) -> ObservedInput {
    let captured = CapturedBackend::new(
        [],
        BTreeMap::from([(identity.to_string(), input.content.stable_bytes())]),
    );
    ObservedInput {
        display_name: input.display_name.clone(),
        fingerprint: Fingerprint::new(captured.input_fingerprint().as_str()),
    }
}

fn invalid_observation_fingerprint(sample: &BackendSample) -> Fingerprint {
    let mut source = INVALID_OBSERVATION_PROTOCOL.to_vec();
    push_field(&mut source, &sample.source.content.stable_bytes());
    for diagnostic in sample.diagnostics.iter() {
        push_field(&mut source, &diagnostic.stable_bytes());
    }
    let assets = sample
        .assets
        .iter()
        .map(|(spelling, input)| (spelling.clone(), input.content.stable_bytes()))
        .collect();
    let synthetic = CapturedBackend::new(source, assets);
    Fingerprint::new(synthetic.input_fingerprint().as_str())
}

fn stable_sample(mut sample: impl FnMut() -> BackendSample) -> BackendSample {
    let mut previous = sample();
    for _ in 1..MAX_STABILITY_SAMPLES {
        let current = sample();
        if current == previous {
            return current;
        }
        previous = current;
    }
    previous.diagnostics.push(
        BackendDiagnostic::new(
            BackendDiagnosticCode::UnstableInputs,
            format!(
                "backend inputs did not remain unchanged across {MAX_STABILITY_SAMPLES} consecutive samples"
            ),
        )
        .at_path(previous.source.requested_path.clone()),
    );
    previous
}

fn sample_backend(layout: &ProjectLayout) -> BackendSample {
    let source_requested = layout.root.join(layout.backend_entry.relative().as_path());
    let mut sample = BackendSample::new(layout, source_requested.clone());

    let canonical_root = match fs::canonicalize(&layout.root) {
        Ok(path) => path,
        Err(error) => {
            return sample.source_failure(
                BackendDiagnosticCode::Io,
                format!("could not resolve project root: {error}"),
            );
        }
    };
    if !canonical_root.is_dir() {
        return sample.source_failure(
            BackendDiagnosticCode::WrongEntryKind,
            "project root is not a directory",
        );
    }

    let backend_root_requested = canonical_root.join(layout.backend_root.relative().as_path());
    let canonical_backend_root = match fs::canonicalize(&backend_root_requested) {
        Ok(path) => path,
        Err(error) => {
            return sample.source_failure(
                BackendDiagnosticCode::Io,
                format!("could not resolve configured backend root: {error}"),
            );
        }
    };
    if !canonical_backend_root.starts_with(&canonical_root) {
        return sample.source_failure(
            BackendDiagnosticCode::PathEscape,
            "configured backend root resolves outside the project root",
        );
    }
    if !canonical_backend_root.is_dir() {
        return sample.source_failure(
            BackendDiagnosticCode::WrongEntryKind,
            "configured backend root is not a directory",
        );
    }

    let source_directory_requested = source_requested
        .parent()
        .unwrap_or(canonical_backend_root.as_path());
    let canonical_source_directory = match fs::canonicalize(source_directory_requested) {
        Ok(path) => path,
        Err(error) => {
            return sample.source_failure(
                BackendDiagnosticCode::Io,
                format!("could not resolve backend source directory: {error}"),
            );
        }
    };
    if !canonical_source_directory.starts_with(&canonical_backend_root) {
        return sample.source_failure(
            BackendDiagnosticCode::PathEscape,
            "backend source directory resolves outside the configured backend root",
        );
    }

    let canonical_source = match fs::canonicalize(&source_requested) {
        Ok(path) => path,
        Err(error) => {
            return sample.source_failure(
                BackendDiagnosticCode::Io,
                format!("could not resolve backend entry: {error}"),
            );
        }
    };
    if !canonical_source.starts_with(&canonical_backend_root) {
        return sample.source_failure(
            BackendDiagnosticCode::PathEscape,
            "backend entry resolves outside the configured backend root",
        );
    }
    if !canonical_source.is_file() {
        return sample.source_failure(
            BackendDiagnosticCode::WrongEntryKind,
            "configured backend entry is not a regular file",
        );
    }

    let source = match fs::read(&canonical_source) {
        Ok(source) => source,
        Err(error) => {
            return sample.source_failure(
                BackendDiagnosticCode::Io,
                format!("could not read backend entry: {error}"),
            );
        }
    };
    sample.source = SampleInput {
        display_name: layout.backend_entry.relative().as_str().to_string(),
        requested_path: source_requested.clone(),
        canonical_path: Some(canonical_source),
        content: InputContent::Bytes(source.clone()),
    };

    let source_text = match std::str::from_utf8(&source) {
        Ok(source) => source,
        Err(error) => {
            sample.diagnostics.push(
                BackendDiagnostic::new(
                    BackendDiagnosticCode::InvalidUtf8,
                    format!("backend entry is not UTF-8: {error}"),
                )
                .at_path(source_requested),
            );
            return sample;
        }
    };
    let contract = match spock_lang::compile(source_text) {
        Ok(contract) => contract,
        Err(diagnostics) => {
            for diagnostic in diagnostics {
                sample.diagnostics.push(BackendDiagnostic {
                    code: BackendDiagnosticCode::Language,
                    message: diagnostic.message,
                    path: Some(source_requested.clone()),
                    span: Some(diagnostic.span.start..diagnostic.span.end),
                    language_code: Some(diagnostic.code),
                });
            }
            return sample;
        }
    };

    let asset_spellings = contract
        .seed
        .iter()
        .flat_map(|row| row.fields.values())
        .filter_map(|value| match value {
            SeedValue::File { path } => Some(path.clone()),
            _ => None,
        })
        .collect::<BTreeSet<_>>();

    for spelling in asset_spellings {
        let display_name = format!("seed asset `{spelling}`");
        let requested = source_directory_requested.join(Path::new(&spelling));
        let canonical = match fs::canonicalize(&requested) {
            Ok(path) => path,
            Err(error) => {
                let message = format!("could not resolve seed asset `{spelling}`: {error}");
                sample.assets.insert(
                    spelling,
                    SampleInput::unavailable(display_name, requested.clone(), &message),
                );
                sample.diagnostics.push(
                    BackendDiagnostic::new(BackendDiagnosticCode::Io, message).at_path(requested),
                );
                continue;
            }
        };
        if !canonical.starts_with(&canonical_source_directory)
            || !canonical.starts_with(&canonical_backend_root)
            || !canonical.starts_with(&canonical_root)
        {
            let message =
                format!("seed asset `{spelling}` resolves outside the backend source directory");
            let mut input =
                SampleInput::unavailable(display_name, requested.clone(), message.clone());
            input.canonical_path = Some(canonical);
            sample.assets.insert(spelling, input);
            sample.diagnostics.push(
                BackendDiagnostic::new(BackendDiagnosticCode::PathEscape, message)
                    .at_path(requested),
            );
            continue;
        }
        if !canonical.is_file() {
            let message = format!("seed asset `{spelling}` is not a regular file");
            let mut input =
                SampleInput::unavailable(display_name, requested.clone(), message.clone());
            input.canonical_path = Some(canonical);
            sample.assets.insert(spelling, input);
            sample.diagnostics.push(
                BackendDiagnostic::new(BackendDiagnosticCode::WrongEntryKind, message)
                    .at_path(requested),
            );
            continue;
        }
        match fs::read(&canonical) {
            Ok(bytes) => {
                sample.assets.insert(
                    spelling,
                    SampleInput {
                        display_name,
                        requested_path: requested,
                        canonical_path: Some(canonical),
                        content: InputContent::Bytes(bytes),
                    },
                );
            }
            Err(error) => {
                let message = format!("could not read seed asset `{spelling}`: {error}");
                let mut input =
                    SampleInput::unavailable(display_name, requested.clone(), message.clone());
                input.canonical_path = Some(canonical);
                sample.assets.insert(spelling, input);
                sample.diagnostics.push(
                    BackendDiagnostic::new(BackendDiagnosticCode::Io, message).at_path(requested),
                );
            }
        }
    }

    sample
}

fn push_field(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
    output.extend_from_slice(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use spock_project::{load_project_from, ProjectManifest, MANIFEST_FILE};
    use tempfile::tempdir;

    const STORAGE_SOURCE: &str = "auth table user { key id: uuid = auto\n \
        username: text unique\n avatar: storage_object? }\n\
        seed { user { username: \"u\", avatar: file(\"./seed/pic.png\") } }\n";

    fn project(source: &[u8]) -> (tempfile::TempDir, ProjectLayout) {
        let temp = tempdir().expect("temp project");
        fs::create_dir(temp.path().join("backend")).expect("backend directory");
        fs::write(temp.path().join("backend/app.spock"), source).expect("backend source");
        let manifest = ProjectManifest::new("demo", "backend", "app.spock", None)
            .expect("manifest")
            .to_toml_string();
        fs::write(temp.path().join(MANIFEST_FILE), manifest).expect("project manifest");
        let layout = load_project_from(temp.path()).expect("project layout");
        (temp, layout)
    }

    #[test]
    fn empty_backend_captures_without_constructing_a_runtime_generation() {
        let (_temp, layout) = project(b"// intentionally empty\n");

        let observation = observe_backend(&layout);

        assert!(observation.is_valid(), "{}", observation.diagnostics());
        let captured = observation.captured_backend().expect("captured backend");
        assert_eq!(captured.source(), b"// intentionally empty\n");
        assert_eq!(
            observation.fingerprint().as_str(),
            captured.input_fingerprint().as_str()
        );
    }

    #[test]
    fn seed_assets_are_captured_and_participate_in_change_detection() {
        let (temp, layout) = project(STORAGE_SOURCE.as_bytes());
        fs::create_dir(temp.path().join("backend/seed")).expect("seed directory");
        let asset = temp.path().join("backend/seed/pic.png");
        fs::write(&asset, b"first").expect("first asset");
        let first = observe_backend(&layout);

        fs::write(&asset, b"second").expect("second asset");
        let second = observe_backend(&layout);

        assert!(first.is_valid(), "{}", first.diagnostics());
        assert!(second.is_valid(), "{}", second.diagnostics());
        assert_eq!(
            first
                .captured_backend()
                .expect("first capture")
                .seed_asset("./seed/pic.png"),
            Some(b"first".as_slice())
        );
        assert_ne!(first.fingerprint(), second.fingerprint());
        assert_eq!(
            second.changed_inputs_since(&first),
            vec!["seed asset `./seed/pic.png`"]
        );
    }

    #[test]
    fn stable_invalid_source_has_diagnostics_and_a_recoverable_identity() {
        let (temp, layout) = project(b"table broken {");
        let invalid = observe_backend(&layout);

        assert!(!invalid.is_valid());
        assert!(invalid
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == BackendDiagnosticCode::Language));

        fs::write(
            temp.path().join("backend/app.spock"),
            b"table fixed { key id: uuid = auto }",
        )
        .expect("fixed source");
        let fixed = observe_backend(&layout);
        assert!(fixed.is_valid(), "{}", fixed.diagnostics());
        assert_ne!(invalid.fingerprint(), fixed.fingerprint());
        assert_eq!(
            fixed.changed_inputs_since(&invalid),
            vec!["backend/app.spock"]
        );
    }

    #[test]
    fn missing_seed_asset_is_invalid_and_recovers_when_the_file_appears() {
        let (temp, layout) = project(STORAGE_SOURCE.as_bytes());
        fs::create_dir(temp.path().join("backend/seed")).expect("seed directory");
        let missing = observe_backend(&layout);
        assert!(!missing.is_valid());

        fs::write(temp.path().join("backend/seed/pic.png"), b"payload").expect("seed asset");
        let recovered = observe_backend(&layout);

        assert!(recovered.is_valid(), "{}", recovered.diagnostics());
        assert_ne!(missing.fingerprint(), recovered.fingerprint());
        assert_eq!(
            recovered.changed_inputs_since(&missing),
            vec!["seed asset `./seed/pic.png`"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn seed_asset_symlink_cannot_escape_the_source_directory() {
        use std::os::unix::fs::symlink;

        let (temp, layout) = project(STORAGE_SOURCE.as_bytes());
        let outside = tempdir().expect("outside directory");
        fs::write(outside.path().join("pic.png"), b"outside").expect("outside asset");
        fs::create_dir(temp.path().join("backend/seed")).expect("seed directory");
        symlink(
            outside.path().join("pic.png"),
            temp.path().join("backend/seed/pic.png"),
        )
        .expect("escaping symlink");

        let observation = observe_backend(&layout);

        assert!(!observation.is_valid());
        assert!(observation
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == BackendDiagnosticCode::PathEscape));
    }

    #[test]
    fn changing_samples_are_not_published_as_a_coherent_capture() {
        let (_temp, layout) = project(b"");
        let source_path = layout.root.join(layout.backend_entry.relative().as_path());
        let samples = [b"a", b"b", b"c", b"d"].map(|bytes| BackendSample {
            source: SampleInput {
                display_name: "backend/app.spock".into(),
                requested_path: source_path.clone(),
                canonical_path: Some(source_path.clone()),
                content: InputContent::Bytes(bytes.to_vec()),
            },
            assets: BTreeMap::new(),
            diagnostics: BackendDiagnostics::default(),
        });
        let mut samples = samples.into_iter();

        let sample = stable_sample(|| samples.next().expect("bounded sample"));

        assert!(sample
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == BackendDiagnosticCode::UnstableInputs));
    }
}

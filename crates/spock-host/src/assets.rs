use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{BufReader, Read},
    path::{Component, Path, PathBuf},
};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use uhura_host::WebAssets;

pub const SPOCK_UHURA_WEB_DIST: &str = "SPOCK_UHURA_WEB_DIST";
pub const SPOCK_UHURA_WASM_DIST: &str = "SPOCK_UHURA_WASM_DIST";

// This value is captured by rustc, not read from the process environment.
// Official distribution builds bind it to the exact sidecar manifest produced
// earlier in the same release workflow. Source/test builds intentionally leave
// it unset and use the explicit paired asset-root override above.
const PACKAGED_UHURA_MANIFEST_SHA256: Option<&str> =
    option_env!("SPOCK_PACKAGED_UHURA_MANIFEST_SHA256");
const SIDECAR_PROTOCOL: &str = "spock-asset-sidecar/1";
const HOST_ENVIRONMENT_PROTOCOL: &str = "spock-host-environment/1";
const PROJECT_STATUS_PROTOCOL: &str = "spock-project-status/1";
const PROJECT_EVENT_PROTOCOL: &str = "spock-project-event/1";
const EDITOR_STATE_PROTOCOL: &str = "uhura-editor-state/1";
const EDITOR_EVENT_PROTOCOL: &str = "uhura-editor-event/0";
const IR_PROTOCOL: &str = "uhura-ir/0";
const INSPECT_PROTOCOL: &str = "uhura-inspect/0";
const VIEW_PROTOCOL: &str = "uhura-view/0";
const PROVIDER_PROTOCOL: &str = "uhura-provider/0";
const SIDECAR_PROTOCOLS: [(&str, &str); 9] = [
    ("environment", HOST_ENVIRONMENT_PROTOCOL),
    ("project_status", PROJECT_STATUS_PROTOCOL),
    ("project_event", PROJECT_EVENT_PROTOCOL),
    ("editor_state", EDITOR_STATE_PROTOCOL),
    ("editor_event", EDITOR_EVENT_PROTOCOL),
    ("ir", IR_PROTOCOL),
    ("inspect", INSPECT_PROTOCOL),
    ("view", VIEW_PROTOCOL),
    ("provider", PROVIDER_PROTOCOL),
];
const MAX_MANIFEST_BYTES: u64 = 8 * 1024 * 1024;
const REQUIRED_FILES: [&str; 3] = [
    "web/index.html",
    "wasm/uhura_wasm.js",
    "wasm/uhura_wasm_bg.wasm",
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UhuraAssetRoots {
    pub web: PathBuf,
    pub wasm: PathBuf,
}

impl UhuraAssetRoots {
    pub fn load(&self) -> Result<WebAssets, AssetError> {
        WebAssets::from_directories(&self.web, &self.wasm).map_err(|message| {
            AssetError::InvalidBundle {
                roots: self.clone(),
                message,
            }
        })
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    #[error("{variable} must be set together with {other}")]
    PartialOverride {
        variable: &'static str,
        other: &'static str,
    },
    #[error(
        "could not locate the packaged Uhura web and Wasm bundles (looked in {attempted}); \
         set {SPOCK_UHURA_WEB_DIST} and {SPOCK_UHURA_WASM_DIST} for a source/test override"
    )]
    NotFound { attempted: String },
    #[error(
        "invalid Uhura asset bundle at web={} wasm={}: {message}",
        roots.web.display(),
        roots.wasm.display()
    )]
    InvalidBundle {
        roots: UhuraAssetRoots,
        message: String,
    },
    #[error("invalid packaged Uhura asset sidecar: {message}")]
    InvalidSidecar { message: String },
    #[error("could not resolve the current executable while locating Uhura assets: {0}")]
    Executable(std::io::Error),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SidecarManifest {
    protocol: String,
    spock_commit: String,
    uhura_commit: String,
    protocols: BTreeMap<String, String>,
    files: Vec<SidecarFile>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SidecarFile {
    path: String,
    sha256: String,
    size: u64,
}

#[derive(Debug, Eq, PartialEq)]
struct ObservedFile {
    path: String,
    sha256: String,
    size: u64,
}

struct LocatedUhuraAssets {
    roots: UhuraAssetRoots,
    manifest: Option<SidecarManifest>,
}

/// The reusable `uhura-host` never searches a checkout. This aggregate-host
/// adapter checks an explicit paired override, executable-relative package
/// locations. Source builds opt in through the paired environment override;
/// `uhura-host` itself never searches a checkout.
///
/// Load the exact package-owned bytes whose manifest is bound to this
/// executable and whose inventory matches that manifest. Explicit development
/// overrides have no package identity and are still captured once into the
/// same immutable [`WebAssets`] representation.
///
/// The executable binding detects a changed or coherently replaced sidecar
/// while the binary remains trusted. It is not a signature over the binary and
/// cannot defend against replacement of both artifacts or a compromised build.
pub fn load_uhura_assets() -> Result<WebAssets, AssetError> {
    let located = locate_uhura_asset_source()?;
    let assets = located.roots.load()?;
    if let Some(manifest) = located.manifest {
        validate_loaded_assets(&manifest, &assets)
            .map_err(|message| AssetError::InvalidSidecar { message })?;
    }
    Ok(assets)
}

fn locate_uhura_asset_source() -> Result<LocatedUhuraAssets, AssetError> {
    let web_override = std::env::var_os(SPOCK_UHURA_WEB_DIST).map(PathBuf::from);
    let wasm_override = std::env::var_os(SPOCK_UHURA_WASM_DIST).map(PathBuf::from);
    match (web_override, wasm_override) {
        (Some(web), Some(wasm)) => {
            return Ok(LocatedUhuraAssets {
                roots: UhuraAssetRoots { web, wasm },
                manifest: None,
            });
        }
        (Some(_), None) => {
            return Err(AssetError::PartialOverride {
                variable: SPOCK_UHURA_WEB_DIST,
                other: SPOCK_UHURA_WASM_DIST,
            });
        }
        (None, Some(_)) => {
            return Err(AssetError::PartialOverride {
                variable: SPOCK_UHURA_WASM_DIST,
                other: SPOCK_UHURA_WEB_DIST,
            });
        }
        (None, None) => {}
    }

    let executable = std::env::current_exe().map_err(AssetError::Executable)?;
    let mut candidates = Vec::new();
    if let Some(bin) = executable.parent() {
        // Conventional prefix install: <prefix>/bin/spock plus
        // <prefix>/share/spock/uhura/{web,wasm}.
        let root = bin.join("../share/spock/uhura");
        candidates.push((
            root.clone(),
            UhuraAssetRoots {
                web: root.join("web"),
                wasm: root.join("wasm"),
            },
        ));
        // npm: <package>/binaries/<platform>/spock plus one shared sidecar.
        let root = bin.join("../../share/spock/uhura");
        candidates.push((
            root.clone(),
            UhuraAssetRoots {
                web: root.join("web"),
                wasm: root.join("wasm"),
            },
        ));
    }
    let mut attempted = Vec::new();
    let mut invalid = Vec::new();
    for (root, roots) in candidates {
        if attempted.contains(&roots) {
            continue;
        }
        let manifest = root.join("manifest.json");
        let mut candidate_present = false;
        for path in [&manifest, &roots.web, &roots.wasm] {
            match fs::symlink_metadata(path) {
                Ok(_) => candidate_present = true,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => {
                    candidate_present = true;
                    invalid.push(format!("{}: {error}", path.display()));
                }
            }
        }
        if candidate_present {
            match validate_packaged_sidecar(&root, &roots) {
                Ok(manifest) => {
                    return Ok(LocatedUhuraAssets {
                        roots,
                        manifest: Some(manifest),
                    });
                }
                Err(message) => invalid.push(format!("{}: {message}", root.display())),
            }
        }
        attempted.push(roots);
    }

    if !invalid.is_empty() {
        return Err(AssetError::InvalidSidecar {
            message: invalid.join("; "),
        });
    }

    Err(AssetError::NotFound {
        attempted: attempted
            .iter()
            .map(|roots| format!("{} + {}", roots.web.display(), roots.wasm.display()))
            .collect::<Vec<_>>()
            .join(", "),
    })
}

fn validate_packaged_sidecar(
    sidecar_root: &Path,
    roots: &UhuraAssetRoots,
) -> Result<SidecarManifest, String> {
    let expected_manifest_sha256 =
        require_packaged_manifest_sha256(PACKAGED_UHURA_MANIFEST_SHA256)?;
    validate_packaged_sidecar_with_digest(sidecar_root, roots, expected_manifest_sha256)
}

fn validate_packaged_sidecar_with_digest(
    sidecar_root: &Path,
    roots: &UhuraAssetRoots,
    expected_manifest_sha256: &str,
) -> Result<SidecarManifest, String> {
    let root_metadata = fs::symlink_metadata(sidecar_root)
        .map_err(|error| format!("could not inspect {}: {error}", sidecar_root.display()))?;
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return Err(format!(
            "sidecar root {} must be a real directory, not a symlink or special file",
            sidecar_root.display()
        ));
    }

    let expected_roots = UhuraAssetRoots {
        web: sidecar_root.join("web"),
        wasm: sidecar_root.join("wasm"),
    };
    if roots != &expected_roots {
        return Err("asset roots do not belong to the declared sidecar root".to_owned());
    }

    let manifest_path = sidecar_root.join("manifest.json");
    let manifest_metadata = symlink_free_file_metadata(&manifest_path, "manifest")?;
    if manifest_metadata.len() > MAX_MANIFEST_BYTES {
        return Err(format!(
            "manifest is {} bytes; maximum is {MAX_MANIFEST_BYTES}",
            manifest_metadata.len()
        ));
    }
    let manifest_bytes = fs::read(&manifest_path)
        .map_err(|error| format!("could not read {}: {error}", manifest_path.display()))?;
    validate_manifest_binding(&manifest_bytes, expected_manifest_sha256)?;
    let manifest: SidecarManifest = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| format!("could not parse {}: {error}", manifest_path.display()))?;

    validate_manifest_header(&manifest)?;
    validate_manifest_files(&manifest.files)?;

    let mut observed = Vec::new();
    collect_observed_files(sidecar_root, &roots.web, &mut observed)?;
    collect_observed_files(sidecar_root, &roots.wasm, &mut observed)?;
    observed.sort_by(|left, right| left.path.cmp(&right.path));
    validate_case_insensitive_uniqueness(observed.iter().map(|file| file.path.as_str()))?;

    if manifest.files.len() != observed.len() {
        return Err(format!(
            "manifest lists {} files but the sidecar contains {} files",
            manifest.files.len(),
            observed.len()
        ));
    }
    for (declared, actual) in manifest.files.iter().zip(&observed) {
        if declared.path != actual.path {
            return Err(format!(
                "file inventory mismatch: manifest has {} where sidecar has {}",
                declared.path, actual.path
            ));
        }
        if declared.size != actual.size {
            return Err(format!(
                "size mismatch for {}: manifest has {} but sidecar has {}",
                declared.path, declared.size, actual.size
            ));
        }
        if declared.sha256 != actual.sha256 {
            return Err(format!(
                "SHA-256 mismatch for {}: manifest has {} but sidecar has {}",
                declared.path, declared.sha256, actual.sha256
            ));
        }
    }

    for required in REQUIRED_FILES {
        if observed
            .binary_search_by_key(&required, |file| file.path.as_str())
            .is_err()
        {
            return Err(format!("required sidecar file {required} is missing"));
        }
    }

    Ok(manifest)
}

fn require_packaged_manifest_sha256(configured: Option<&str>) -> Result<&str, String> {
    let configured = configured.ok_or_else(|| {
        format!(
            "this executable has no trusted Uhura sidecar manifest identity; packaged sidecar loading is disabled (source/test builds must set the paired {SPOCK_UHURA_WEB_DIST} and {SPOCK_UHURA_WASM_DIST} overrides)"
        )
    })?;
    if !lowercase_sha256(configured) {
        return Err(
            "the executable's trusted Uhura sidecar manifest identity is not a 64-character lowercase SHA-256 digest"
                .to_owned(),
        );
    }
    Ok(configured)
}

fn validate_manifest_binding(
    manifest_bytes: &[u8],
    expected_manifest_sha256: &str,
) -> Result<(), String> {
    if !lowercase_sha256(expected_manifest_sha256) {
        return Err("trusted sidecar manifest SHA-256 is malformed".to_owned());
    }
    let observed = sha256_bytes(manifest_bytes);
    if observed != expected_manifest_sha256 {
        return Err(format!(
            "manifest SHA-256 {observed} does not match executable-bound identity {expected_manifest_sha256}"
        ));
    }
    Ok(())
}

fn lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn sha256_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn validate_loaded_assets(manifest: &SidecarManifest, assets: &WebAssets) -> Result<(), String> {
    let observed = assets.inventory();
    if manifest.files.len() != observed.len() {
        return Err(format!(
            "manifest lists {} files but the immutable asset snapshot contains {} files",
            manifest.files.len(),
            observed.len()
        ));
    }
    for (declared, actual) in manifest.files.iter().zip(&observed) {
        if declared.path != actual.path {
            return Err(format!(
                "file inventory mismatch: manifest has {} where the immutable asset snapshot has {}",
                declared.path, actual.path
            ));
        }
        if declared.size != actual.size {
            return Err(format!(
                "size mismatch for {}: manifest has {} but the immutable asset snapshot has {}",
                declared.path, declared.size, actual.size
            ));
        }
        if declared.sha256 != actual.sha256 {
            return Err(format!(
                "SHA-256 mismatch for {}: manifest has {} but the immutable asset snapshot has {}",
                declared.path, declared.sha256, actual.sha256
            ));
        }
    }
    Ok(())
}

fn validate_manifest_header(manifest: &SidecarManifest) -> Result<(), String> {
    if manifest.protocol != SIDECAR_PROTOCOL {
        return Err(format!(
            "unsupported manifest protocol {}; expected {SIDECAR_PROTOCOL}",
            manifest.protocol
        ));
    }
    for (name, commit) in [
        ("spock_commit", manifest.spock_commit.as_str()),
        ("uhura_commit", manifest.uhura_commit.as_str()),
    ] {
        if commit.len() != 40
            || !commit
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(format!(
                "manifest {name} must be exactly 40 lowercase hexadecimal characters"
            ));
        }
    }
    for (name, expected) in SIDECAR_PROTOCOLS {
        match manifest.protocols.get(name) {
            Some(actual) if actual == expected => {}
            Some(actual) => {
                return Err(format!("protocol {name} is {actual}; expected {expected}"));
            }
            None => return Err(format!("required protocol {name} is missing")),
        }
    }
    for name in manifest.protocols.keys() {
        if !SIDECAR_PROTOCOLS
            .iter()
            .any(|(expected, _)| name == expected)
        {
            return Err(format!("unsupported protocol key {name}"));
        }
    }
    Ok(())
}

fn validate_manifest_files(files: &[SidecarFile]) -> Result<(), String> {
    let mut previous: Option<&str> = None;
    for file in files {
        validate_relative_asset_path(&file.path)?;
        if let Some(previous) = previous {
            if previous >= file.path.as_str() {
                let reason = if previous == file.path {
                    "contains a duplicate"
                } else {
                    "is not sorted"
                };
                return Err(format!("manifest file inventory {reason} at {}", file.path));
            }
        }
        previous = Some(&file.path);
        if file.size == 0 {
            return Err(format!("manifest size for {} must be positive", file.path));
        }
        if file.sha256.len() != 64
            || !file
                .sha256
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(format!(
                "manifest SHA-256 for {} must be 64 lowercase hexadecimal characters",
                file.path
            ));
        }
    }
    validate_case_insensitive_uniqueness(files.iter().map(|file| file.path.as_str()))
}

fn validate_relative_asset_path(path: &str) -> Result<(), String> {
    if path.is_empty() || path.contains('\\') {
        return Err(format!("unsafe manifest file path {path:?}"));
    }
    let segments = path.split('/').collect::<Vec<_>>();
    if segments.len() < 2
        || !matches!(segments.first(), Some(&"web" | &"wasm"))
        || segments
            .iter()
            .any(|segment| !portable_asset_segment(segment))
        || Path::new(path)
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("unsafe manifest file path {path:?}"));
    }
    Ok(())
}

fn portable_asset_segment(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        && !segment.ends_with('.')
        && !windows_device_segment(segment)
}

fn windows_device_segment(segment: &str) -> bool {
    let stem = segment.split_once('.').map_or(segment, |(stem, _)| stem);
    if stem.eq_ignore_ascii_case("con")
        || stem.eq_ignore_ascii_case("prn")
        || stem.eq_ignore_ascii_case("aux")
        || stem.eq_ignore_ascii_case("nul")
    {
        return true;
    }
    let bytes = stem.as_bytes();
    bytes.len() == 4
        && (bytes[..3].eq_ignore_ascii_case(b"com") || bytes[..3].eq_ignore_ascii_case(b"lpt"))
        && matches!(bytes[3], b'1'..=b'9')
}

fn validate_case_insensitive_uniqueness<'a>(
    paths: impl Iterator<Item = &'a str>,
) -> Result<(), String> {
    let mut folded = BTreeSet::new();
    for path in paths {
        if !folded.insert(path.to_lowercase()) {
            return Err(format!(
                "sidecar contains a case-insensitive path collision at {path}"
            ));
        }
    }
    Ok(())
}

fn collect_observed_files(
    sidecar_root: &Path,
    directory: &Path,
    observed: &mut Vec<ObservedFile>,
) -> Result<(), String> {
    let metadata = fs::symlink_metadata(directory)
        .map_err(|error| format!("could not inspect {}: {error}", directory.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "{} must be a real directory, not a symlink or special file",
            directory.display()
        ));
    }

    let entries = fs::read_dir(directory)
        .map_err(|error| format!("could not read {}: {error}", directory.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "could not read an entry in {}: {error}",
                directory.display()
            )
        })?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("could not inspect {}: {error}", path.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "sidecar path {} must not be a symlink",
                path.display()
            ));
        }
        if metadata.is_dir() {
            collect_observed_files(sidecar_root, &path, observed)?;
            continue;
        }
        if !metadata.is_file() {
            return Err(format!(
                "sidecar path {} is not a regular file",
                path.display()
            ));
        }
        if metadata.len() == 0 {
            return Err(format!("sidecar file {} must not be empty", path.display()));
        }
        let relative = path.strip_prefix(sidecar_root).map_err(|_| {
            format!(
                "sidecar path {} escaped root {}",
                path.display(),
                sidecar_root.display()
            )
        })?;
        let relative = manifest_path(relative)?;
        validate_relative_asset_path(&relative)?;
        observed.push(ObservedFile {
            path: relative,
            sha256: hash_file(&path)?,
            size: metadata.len(),
        });
    }
    Ok(())
}

fn manifest_path(relative: &Path) -> Result<String, String> {
    let mut segments = Vec::new();
    for component in relative.components() {
        let Component::Normal(segment) = component else {
            return Err(format!(
                "sidecar path {} is not a safe relative path",
                relative.display()
            ));
        };
        segments.push(
            segment
                .to_str()
                .ok_or_else(|| {
                    format!(
                        "sidecar path {} cannot be represented as UTF-8",
                        relative.display()
                    )
                })?
                .to_owned(),
        );
    }
    Ok(segments.join("/"))
}

fn symlink_free_file_metadata(path: &Path, label: &str) -> Result<fs::Metadata, String> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| format!("could not inspect {label} {}: {error}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "{label} {} must be a regular file, not a symlink",
            path.display()
        ));
    }
    Ok(metadata)
}

fn hash_file(path: &Path) -> Result<String, String> {
    let file = fs::File::open(path)
        .map_err(|error| format!("could not open {} for hashing: {error}", path.display()))?;
    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader
            .read(&mut buffer)
            .map_err(|error| format!("could not hash {}: {error}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{json, Value};
    use tempfile::tempdir;

    // Most validation tests deliberately mutate one manifest field and need
    // the executable binding to follow that fixture so they can reach the
    // lower-level protocol or inventory assertion under test.
    fn validate_packaged_sidecar(
        sidecar_root: &Path,
        roots: &UhuraAssetRoots,
    ) -> Result<SidecarManifest, String> {
        let expected = hash_file(&sidecar_root.join("manifest.json"))?;
        validate_packaged_sidecar_with_digest(sidecar_root, roots, &expected)
    }

    #[test]
    fn explicit_asset_roots_are_snapshotted_as_one_bundle() {
        let temp = tempdir().expect("temporary asset root");
        let web = temp.path().join("web");
        let wasm = temp.path().join("wasm");
        std::fs::create_dir_all(web.join("assets")).expect("web assets");
        std::fs::create_dir_all(&wasm).expect("wasm directory");
        std::fs::write(
            web.join("index.html"),
            r#"<!doctype html><script type="module" src="/assets/app.js"></script>"#,
        )
        .expect("web index");
        std::fs::write(web.join("assets/app.js"), "export {};\n").expect("web script");
        std::fs::write(wasm.join("uhura_wasm.js"), "export {};\n").expect("wasm loader");
        std::fs::write(wasm.join("uhura_wasm_bg.wasm"), b"wasm").expect("wasm module");

        let roots = UhuraAssetRoots { web, wasm };
        let assets = roots.load().expect("valid immutable web assets");
        drop(assets);
    }

    #[test]
    fn packaged_sidecar_accepts_an_exact_manifest() {
        let fixture = SidecarFixture::new();

        validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect("exact sidecar manifest should validate");
    }

    #[test]
    fn packaged_sidecar_requires_an_executable_bound_manifest_identity() {
        let error = require_packaged_manifest_sha256(None)
            .expect_err("a source binary must not accept an executable-relative sidecar");
        assert!(
            error.contains("no trusted Uhura sidecar manifest identity"),
            "{error}"
        );

        let error = require_packaged_manifest_sha256(Some("not-a-digest"))
            .expect_err("a malformed build identity must fail closed");
        assert!(
            error.contains("not a 64-character lowercase SHA-256"),
            "{error}"
        );
    }

    #[test]
    fn executable_binding_rejects_a_coherently_rehashed_sidecar() {
        let mut fixture = SidecarFixture::new();
        let trusted = fixture.manifest_sha256();

        fs::write(
            fixture.root.join("web/assets/app.js"),
            "export const coherently_rehashed = true;\n",
        )
        .expect("replace asset bytes");
        fixture.refresh_manifest_entry("web/assets/app.js");
        fixture.write_manifest();

        validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect("the rewritten manifest remains internally consistent");
        let error = validate_packaged_sidecar_with_digest(&fixture.root, &fixture.roots, &trusted)
            .expect_err("the executable must retain the original manifest identity");
        assert!(
            error.contains("does not match executable-bound identity"),
            "{error}"
        );
    }

    #[test]
    fn packaged_sidecar_rejects_protocol_mismatches() {
        let mut fixture = SidecarFixture::new();
        fixture.manifest["protocol"] = json!("spock-asset-sidecar/2");
        fixture.write_manifest();

        let error = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect_err("unknown sidecar protocol must fail closed");
        assert!(error.contains("unsupported manifest protocol"), "{error}");

        fixture.manifest["protocol"] = json!(SIDECAR_PROTOCOL);
        fixture.manifest["protocols"]["project_event"] = json!("wrong/1");
        fixture.write_manifest();
        let error = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect_err("framework protocol mismatch must fail closed");
        assert!(error.contains("protocol project_event"), "{error}");

        let mut fixture = SidecarFixture::new();
        fixture.manifest["protocols"]
            .as_object_mut()
            .expect("protocol object")
            .remove("provider");
        fixture.write_manifest();
        let error = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect_err("missing protocol must fail closed");
        assert!(
            error.contains("required protocol provider is missing"),
            "{error}"
        );

        let mut fixture = SidecarFixture::new();
        fixture.manifest["protocols"]["future"] = json!("future/1");
        fixture.write_manifest();
        let error = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect_err("unknown protocol key must fail closed");
        assert!(error.contains("unsupported protocol key future"), "{error}");
    }

    #[test]
    fn packaged_sidecar_requires_full_git_object_ids() {
        let mut fixture = SidecarFixture::new();
        fixture.manifest["spock_commit"] = json!("unknown");
        fixture.write_manifest();
        let error = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect_err("non-object commit must fail closed");
        assert!(error.contains("spock_commit must be exactly 40"), "{error}");

        fixture.manifest["spock_commit"] = json!("ABCDEF0123456789abcdef0123456789abcdef01");
        fixture.write_manifest();
        let error = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect_err("uppercase object id must fail closed");
        assert!(error.contains("spock_commit must be exactly 40"), "{error}");
    }

    #[test]
    fn packaged_sidecar_rejects_unsafe_duplicate_and_unsorted_paths() {
        let mut fixture = SidecarFixture::new();
        fixture.manifest["files"][0]["path"] = json!("web/../escape");
        fixture.write_manifest();
        let error = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect_err("unsafe path must fail closed");
        assert!(error.contains("unsafe manifest file path"), "{error}");

        let mut fixture = SidecarFixture::new();
        let duplicate = fixture.manifest["files"][0].clone();
        fixture.manifest["files"]
            .as_array_mut()
            .expect("file array")
            .insert(1, duplicate);
        fixture.write_manifest();
        let error = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect_err("duplicate path must fail closed");
        assert!(error.contains("contains a duplicate"), "{error}");

        let mut fixture = SidecarFixture::new();
        fixture.manifest["files"]
            .as_array_mut()
            .expect("file array")
            .reverse();
        fixture.write_manifest();
        let error = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect_err("unsorted paths must fail closed");
        assert!(error.contains("is not sorted"), "{error}");
    }

    #[test]
    fn packaged_sidecar_paths_use_the_portable_ascii_grammar() {
        for path in [
            "web/café.js",
            "web/.hidden.js",
            "web/-app.js",
            "web/trailing.",
            "web/CON",
            "web/com1.js",
            "wasm/LPT9.bin",
        ] {
            let error = validate_relative_asset_path(path)
                .expect_err("non-portable package path must fail closed");
            assert!(
                error.contains("unsafe manifest file path"),
                "{path}: {error}"
            );
        }
        for path in [
            "web/index.html",
            "web/assets/app-ABC_123.js",
            "wasm/uhura_wasm_bg.wasm",
            "web/console.js",
            "web/com10.js",
        ] {
            validate_relative_asset_path(path)
                .unwrap_or_else(|error| panic!("{path} should be portable: {error}"));
        }
    }

    #[test]
    fn packaged_sidecar_rejects_missing_extra_and_case_colliding_files() {
        let fixture = SidecarFixture::new();
        fs::remove_file(fixture.root.join("web/assets/app.js")).expect("remove declared file");
        let error = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect_err("missing file must fail closed");
        assert!(error.contains("manifest lists"), "{error}");

        let fixture = SidecarFixture::new();
        fs::write(fixture.root.join("web/extra.js"), "extra\n").expect("extra file");
        let error = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect_err("unlisted file must fail closed");
        assert!(error.contains("manifest lists"), "{error}");

        let mut fixture = SidecarFixture::new();
        let mut collision = fixture.manifest["files"]
            .as_array()
            .expect("file array")
            .iter()
            .find(|file| file["path"] == "web/index.html")
            .expect("index manifest entry")
            .clone();
        collision["path"] = json!("web/INDEX.html");
        let files = fixture.manifest["files"]
            .as_array_mut()
            .expect("file array");
        files.push(collision);
        files.sort_by(|left, right| {
            left["path"]
                .as_str()
                .expect("path")
                .cmp(right["path"].as_str().expect("path"))
        });
        fixture.write_manifest();
        let error = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect_err("case-insensitive collision must fail closed");
        assert!(error.contains("case-insensitive path collision"), "{error}");
    }

    #[test]
    fn packaged_sidecar_rejects_size_and_hash_mismatches() {
        let mut fixture = SidecarFixture::new();
        fixture.manifest["files"][0]["size"] = json!(999);
        fixture.write_manifest();
        let error = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect_err("size mismatch must fail closed");
        assert!(error.contains("size mismatch"), "{error}");

        let mut fixture = SidecarFixture::new();
        fixture.manifest["files"][0]["sha256"] =
            json!("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
        fixture.write_manifest();
        let error = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect_err("hash mismatch must fail closed");
        assert!(error.contains("SHA-256 mismatch"), "{error}");
    }

    #[test]
    fn manifest_integrity_covers_the_exact_immutable_bytes_that_will_be_served() {
        let fixture = SidecarFixture::new();
        let manifest = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect("initial sidecar should validate");
        let captured = fixture.roots.load().expect("capture validated assets");

        fs::write(
            fixture.root.join("web/assets/app.js"),
            "export const tampered = true;\n",
        )
        .expect("mutate source after capture");

        validate_loaded_assets(&manifest, &captured)
            .expect("filesystem mutation cannot alter an immutable captured snapshot");
        let tampered = fixture.roots.load().expect("capture mutated assets");
        let error = validate_loaded_assets(&manifest, &tampered)
            .expect_err("mutated captured bytes must fail manifest integrity validation");
        assert!(error.contains("mismatch"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn packaged_sidecar_rejects_symlinks() {
        use std::os::unix::fs::symlink;

        let fixture = SidecarFixture::new();
        symlink(
            fixture.root.join("web/index.html"),
            fixture.root.join("web/link.html"),
        )
        .expect("asset symlink");
        let error = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect_err("asset symlink must fail closed");
        assert!(error.contains("must not be a symlink"), "{error}");

        let fixture = SidecarFixture::new();
        fs::rename(
            fixture.root.join("manifest.json"),
            fixture.root.join("real-manifest.json"),
        )
        .expect("move manifest");
        symlink(
            fixture.root.join("real-manifest.json"),
            fixture.root.join("manifest.json"),
        )
        .expect("manifest symlink");
        let error = validate_packaged_sidecar(&fixture.root, &fixture.roots)
            .expect_err("manifest symlink must fail closed");
        assert!(error.contains("must be a regular file"), "{error}");
    }

    struct SidecarFixture {
        _temp: tempfile::TempDir,
        root: PathBuf,
        roots: UhuraAssetRoots,
        manifest: Value,
    }

    impl SidecarFixture {
        fn new() -> Self {
            let temp = tempdir().expect("temporary sidecar");
            let root = temp.path().join("uhura");
            let roots = UhuraAssetRoots {
                web: root.join("web"),
                wasm: root.join("wasm"),
            };
            fs::create_dir_all(roots.web.join("assets")).expect("web directories");
            fs::create_dir_all(&roots.wasm).expect("wasm directory");
            fs::write(
                roots.web.join("index.html"),
                r#"<!doctype html><script type="module" src="/assets/app.js"></script>"#,
            )
            .expect("web index");
            fs::write(roots.web.join("assets/app.js"), "export {};\n").expect("web script");
            fs::write(roots.wasm.join("uhura_wasm.js"), "export {};\n").expect("wasm loader");
            fs::write(roots.wasm.join("uhura_wasm_bg.wasm"), b"wasm").expect("wasm module");

            let mut files = Vec::new();
            for relative in [
                "wasm/uhura_wasm.js",
                "wasm/uhura_wasm_bg.wasm",
                "web/assets/app.js",
                "web/index.html",
            ] {
                let path = root.join(relative);
                files.push(json!({
                    "path": relative,
                    "sha256": hash_file(&path).expect("fixture hash"),
                    "size": fs::metadata(path).expect("fixture metadata").len(),
                }));
            }
            files.sort_by(|left, right| {
                left["path"]
                    .as_str()
                    .expect("path")
                    .cmp(right["path"].as_str().expect("path"))
            });
            let manifest = json!({
                "protocol": SIDECAR_PROTOCOL,
                "spock_commit": "0123456789abcdef0123456789abcdef01234567",
                "uhura_commit": "89abcdef0123456789abcdef0123456789abcdef",
                "protocols": {
                    "environment": HOST_ENVIRONMENT_PROTOCOL,
                    "project_status": PROJECT_STATUS_PROTOCOL,
                    "project_event": PROJECT_EVENT_PROTOCOL,
                    "editor_state": EDITOR_STATE_PROTOCOL,
                    "editor_event": EDITOR_EVENT_PROTOCOL,
                    "ir": IR_PROTOCOL,
                    "inspect": INSPECT_PROTOCOL,
                    "view": VIEW_PROTOCOL,
                    "provider": PROVIDER_PROTOCOL
                },
                "files": files,
            });
            let fixture = Self {
                _temp: temp,
                root,
                roots,
                manifest,
            };
            fixture.write_manifest();
            fixture
        }

        fn write_manifest(&self) {
            fs::write(
                self.root.join("manifest.json"),
                serde_json::to_vec_pretty(&self.manifest).expect("serialize fixture manifest"),
            )
            .expect("write fixture manifest");
        }

        fn manifest_sha256(&self) -> String {
            hash_file(&self.root.join("manifest.json")).expect("hash fixture manifest")
        }

        fn refresh_manifest_entry(&mut self, relative: &str) {
            let path = self.root.join(relative);
            let entry = self.manifest["files"]
                .as_array_mut()
                .expect("file array")
                .iter_mut()
                .find(|entry| entry["path"] == relative)
                .expect("manifest entry");
            entry["sha256"] = json!(hash_file(&path).expect("asset hash"));
            entry["size"] = json!(fs::metadata(path).expect("asset metadata").len());
        }
    }
}

use std::path::{Path, PathBuf};

use uhura_host::WebAssets;

pub const SPOCK_UHURA_WEB_DIST: &str = "SPOCK_UHURA_WEB_DIST";
pub const SPOCK_UHURA_WASM_DIST: &str = "SPOCK_UHURA_WASM_DIST";

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
    #[error("could not resolve the current executable while locating Uhura assets: {0}")]
    Executable(std::io::Error),
}

/// Locate the framework-owned Uhura sidecars.
///
/// The reusable `uhura-host` never searches a checkout. This aggregate-host
/// adapter checks an explicit paired override, executable-relative package
/// locations, then the compile-time source checkout for developer builds.
pub fn locate_uhura_assets() -> Result<UhuraAssetRoots, AssetError> {
    let web_override = std::env::var_os(SPOCK_UHURA_WEB_DIST).map(PathBuf::from);
    let wasm_override = std::env::var_os(SPOCK_UHURA_WASM_DIST).map(PathBuf::from);
    match (web_override, wasm_override) {
        (Some(web), Some(wasm)) => return Ok(UhuraAssetRoots { web, wasm }),
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
        candidates.push(UhuraAssetRoots {
            web: bin.join("../share/spock/uhura/web"),
            wasm: bin.join("../share/spock/uhura/wasm"),
        });
        // npm: <package>/binaries/<platform>/spock plus one shared sidecar.
        candidates.push(UhuraAssetRoots {
            web: bin.join("../../share/spock/uhura/web"),
            wasm: bin.join("../../share/spock/uhura/wasm"),
        });
    }
    let sidecar = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../uhura/.spock-sidecar");
    candidates.push(UhuraAssetRoots {
        web: sidecar.join("web"),
        wasm: sidecar.join("wasm"),
    });

    let source_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../uhura");
    candidates.push(UhuraAssetRoots {
        web: source_root.join("web/dist"),
        wasm: source_root.join("crates/uhura-wasm/pkg/web"),
    });

    let mut attempted = Vec::new();
    for roots in candidates {
        if attempted.contains(&roots) {
            continue;
        }
        if roots.web.join("index.html").is_file()
            && roots.wasm.join("uhura_wasm.js").is_file()
            && roots.wasm.join("uhura_wasm_bg.wasm").is_file()
        {
            return Ok(roots);
        }
        attempted.push(roots);
    }

    Err(AssetError::NotFound {
        attempted: attempted
            .iter()
            .map(|roots| format!("{} + {}", roots.web.display(), roots.wasm.display()))
            .collect::<Vec<_>>()
            .join(", "),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

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
}

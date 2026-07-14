//! Framework project topology without language semantics or live resources.
//!
//! This crate owns the strict `spock.toml` v1 shape, nearest-root discovery,
//! contained path resolution, deterministic CLI target selection, and
//! mutation-free scaffold/adoption plans. Spock and Uhura remain responsible
//! for enumerating and capturing their own semantic inputs.

#![forbid(unsafe_code)]

mod diagnostic;
mod discovery;
mod layout;
mod manifest;
mod path;
mod plan;
mod starter;

pub use diagnostic::{Diagnostic, DiagnosticCode, Diagnostics, ProjectResult};
pub use discovery::{discover_project_root, resolve_target, ProjectRoot, ResolvedTarget};
pub use layout::{load_project, load_project_from, ClientLayout, ProjectLayout};
pub use manifest::{
    parse_manifest, parse_manifest_file, BackendConfig, ClientConfig, ProjectManifest, ProjectName,
    MANIFEST_FILE, MANIFEST_VERSION,
};
pub use path::{resolve_contained, ContainedPath, NormalizedRelativePath, PathValidationError};
pub use plan::{
    adoption_plan, scaffold_plan, ClientTemplate, InventoryEntryKind, PlanKind, PlannedWrite,
    ProjectInventory, TemplateFile, WritePlan, DEFAULT_BACKEND_SOURCE,
};
pub use starter::minimal_uhura_client_template;

use crate::plan::{ClientTemplate, TemplateFile};

/// The canonical, dependency-free Uhura client created by `spock new`.
///
/// Uhura owns the meaning of these files. This crate deliberately embeds and
/// copies their bytes without parsing, rewriting, or otherwise interpreting
/// them. Keeping the starter here gives the framework CLI one versioned,
/// deterministic source for a full-stack scaffold while `scaffold_plan(...,
/// None)` remains the backend-only path.
pub fn minimal_uhura_client_template() -> ClientTemplate {
    let files = [
        (
            "app/home/page.examples.uhura",
            include_bytes!("../templates/minimal-client/app/home/page.examples.uhura").as_slice(),
        ),
        (
            "app/home/page.uhura",
            include_bytes!("../templates/minimal-client/app/home/page.uhura").as_slice(),
        ),
        (
            "catalog/base.toml",
            include_bytes!("../templates/minimal-client/catalog/base.toml").as_slice(),
        ),
        (
            "fixtures/empty.toml",
            include_bytes!("../templates/minimal-client/fixtures/empty.toml").as_slice(),
        ),
        (
            "fixtures/scripts/empty.toml",
            include_bytes!("../templates/minimal-client/fixtures/scripts/empty.toml").as_slice(),
        ),
        (
            "uhura.toml",
            include_bytes!("../templates/minimal-client/uhura.toml").as_slice(),
        ),
    ];

    let files = files
        .into_iter()
        .map(|(path, contents)| {
            TemplateFile::new(path, contents)
                .expect("canonical Uhura starter paths are valid project-relative files")
        })
        .collect();
    ClientTemplate::new(files)
        .expect("canonical Uhura starter contains one root manifest and unique files")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::{scaffold_plan, DEFAULT_BACKEND_SOURCE};

    const TEMPLATE_PATHS: [&str; 6] = [
        "app/home/page.examples.uhura",
        "app/home/page.uhura",
        "catalog/base.toml",
        "fixtures/empty.toml",
        "fixtures/scripts/empty.toml",
        "uhura.toml",
    ];

    #[test]
    fn embedded_template_is_complete_and_deterministic() {
        let first = minimal_uhura_client_template();
        let second = minimal_uhura_client_template();

        assert_eq!(first, second);
        assert_eq!(
            first
                .files()
                .iter()
                .map(|file| file.path().as_str())
                .collect::<Vec<_>>(),
            TEMPLATE_PATHS
        );
        assert!(first.files().iter().all(|file| !file.contents().is_empty()));
    }

    #[test]
    fn full_stack_scaffold_copies_embedded_bytes_under_client_root() {
        let template = minimal_uhura_client_template();
        let plan = scaffold_plan("demo", "demo", Some(&template)).unwrap();

        for file in template.files() {
            let planned_path = format!("client/{}", file.path());
            assert_eq!(
                plan.write(&planned_path)
                    .map(|write| write.contents.as_slice()),
                Some(file.contents()),
                "scaffold changed opaque Uhura bytes at {planned_path}"
            );
        }
        assert_eq!(
            plan.write("backend/app.spock")
                .map(|write| write.contents.as_slice()),
            Some(DEFAULT_BACKEND_SOURCE.as_bytes())
        );
    }

    #[test]
    fn backend_only_scaffold_still_has_no_client_writes_or_manifest_section() {
        let plan = scaffold_plan("demo", "demo", None).unwrap();

        assert!(plan
            .writes()
            .iter()
            .all(|write| !write.relative_path.as_str().starts_with("client/")));
        let manifest = std::str::from_utf8(&plan.write("spock.toml").unwrap().contents).unwrap();
        assert!(!manifest.contains("[client]"));
    }
}

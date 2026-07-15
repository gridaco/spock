use std::fs;
use std::path::Path;

use spock_project::{
    adoption_plan, load_project_from, minimal_uhura_client_template, resolve_target, scaffold_plan,
    ProjectInventory, ResolvedTarget, WritePlan,
};
use tempfile::tempdir;

fn apply_for_test(plan: &WritePlan) {
    for write in plan.writes() {
        let destination = plan.root.join(write.relative_path.as_path());
        fs::create_dir_all(destination.parent().expect("planned file has parent")).unwrap();
        fs::write(destination, &write.contents).unwrap();
    }
}

#[test]
fn scaffolded_full_stack_project_discovers_and_loads_from_a_descendant() {
    let temp = tempdir().unwrap();
    let destination = temp.path().join("demo");
    let client = minimal_uhura_client_template();
    let plan = scaffold_plan(&destination, "demo", Some(&client)).unwrap();
    plan.preflight(&ProjectInventory::empty(&destination))
        .unwrap();
    apply_for_test(&plan);

    let descendant = destination.join("client/app");
    let target = resolve_target(None, &descendant).unwrap();
    assert!(matches!(target, ResolvedTarget::Project(_)));
    let layout = load_project_from(&descendant).unwrap();
    assert_eq!(layout.manifest.project().as_str(), "demo");
    assert_eq!(
        layout.backend_entry.absolute(),
        &fs::canonicalize(destination.join("backend/app.spock")).unwrap()
    );
    assert!(layout.client.is_some());
}

#[test]
fn adoption_of_existing_sources_writes_only_framework_files() {
    let temp = tempdir().unwrap();
    fs::create_dir_all(temp.path().join("server")).unwrap();
    fs::write(
        temp.path().join("server/app.spock"),
        "// existing backend\n",
    )
    .unwrap();
    fs::create_dir_all(temp.path().join("experience/app")).unwrap();
    fs::write(
        temp.path().join("experience/uhura.toml"),
        "[app]\nname = \"existing\"\n",
    )
    .unwrap();

    let before_backend = fs::read(temp.path().join("server/app.spock")).unwrap();
    let before_client = fs::read(temp.path().join("experience/uhura.toml")).unwrap();
    let inventory = ProjectInventory::scan(temp.path()).unwrap();
    let plan = adoption_plan(&inventory, Some("existing")).unwrap();
    assert_eq!(
        plan.writes()
            .iter()
            .map(|write| write.relative_path.as_str())
            .collect::<Vec<_>>(),
        ["spock.toml"]
    );
    apply_for_test(&plan);

    assert_eq!(
        fs::read(temp.path().join("server/app.spock")).unwrap(),
        before_backend
    );
    assert_eq!(
        fs::read(temp.path().join("experience/uhura.toml")).unwrap(),
        before_client
    );
    let layout = load_project_from(temp.path()).unwrap();
    assert_eq!(layout.manifest.backend().root().as_str(), "server");
    assert_eq!(
        layout.manifest.client().unwrap().root().as_str(),
        "experience"
    );
}

#[test]
fn explicit_spock_target_never_turns_into_project_mode() {
    let temp = tempdir().unwrap();
    let project = temp.path().join("project");
    fs::create_dir_all(&project).unwrap();
    fs::write(project.join("spock.toml"), "not relevant").unwrap();

    let target = resolve_target(Some(Path::new("new-backend.spock")), &project).unwrap();
    assert_eq!(
        target,
        ResolvedTarget::SpockFile(fs::canonicalize(project).unwrap().join("new-backend.spock"))
    );
}

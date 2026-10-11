//! Planning a content install with no network: the required-dependency walk
//! and the `.mrpack` file plan.
//!
//! No mock server appears here, and that is the point -- this is the decision
//! layer, so the tests are documents in and plans out. The documents are
//! shaped like the catalogue's own (`game_versions`, `loaders`, `files[]`,
//! `dependencies[]`), so a change in what the walk reads breaks these tests
//! rather than failing a live run months later.

use std::collections::BTreeMap;

use palantir_loader::dependencies::{
    MAX_DEPTH, Requirement, choose_version, required, resolve_plan,
};
use palantir_loader::error::{Error, Result};
use palantir_loader::mrpack::{SelectedFile, override_layers, plan_files};
use serde_json::{Value, json};

/// A version document as the catalogue shapes it: the two ids, the
/// compatibility lists, and one primary file.
fn version(project: &str, id: &str, game: &[&str], loaders: &[&str], file: &str) -> Value {
    json!({
        "id": id,
        "project_id": project,
        "game_versions": game,
        "loaders": loaders,
        "files": [{
            "filename": file,
            "primary": true,
            "url": format!("https://cdn.example/{file}"),
            "hashes": {"sha1": "0000000000000000000000000000000000000000"},
            "size": 7
        }]
    })
}

/// A compatible version document that also requires other projects.
fn requiring(project: &str, id: &str, requires: Value) -> Value {
    let mut document = version(project, id, &["1.20.1"], &["fabric"], "file.jar");
    document["dependencies"] = requires;
    document
}

/// The catalogue the walk reads, keyed by the id a dependency names.
fn catalogue(entries: Vec<(&str, Vec<Value>)>) -> BTreeMap<String, Vec<Value>> {
    entries
        .into_iter()
        .map(|(id, versions)| (id.to_string(), versions))
        .collect()
}

/// The injected lookup: a fixed catalogue, and a miss that says so.
fn lookup(catalogue: &BTreeMap<String, Vec<Value>>) -> impl FnMut(&str) -> Result<Vec<Value>> + '_ {
    move |key: &str| {
        catalogue.get(key).cloned().ok_or_else(|| Error::Invalid {
            what: "catalogue",
            why: format!("nothing is published for {key}"),
        })
    }
}

/// A raw `modrinth.index.json` as the review step reads it.
fn index(files: Value) -> Value {
    json!({
        "formatVersion": 1,
        "game": "minecraft",
        "versionId": "1.0",
        "name": "A pack",
        "dependencies": {"minecraft": "1.20.1"},
        "files": files
    })
}

/// One index file entry, required on the client unless it says otherwise.
fn entry(path: &str) -> Value {
    json!({
        "path": path,
        "downloads": ["https://cdn.example/f.jar"],
        "hashes": {"sha1": "aa", "sha512": "bb"},
        "fileSize": 3
    })
}

/// The path a planned file will occupy, whichever source it has.
fn planned_path(file: &SelectedFile) -> &str {
    match file {
        SelectedFile::Download { path, .. } | SelectedFile::Local { path } => path,
    }
}

#[test]
fn only_required_dependencies_are_requirements() {
    let document = json!({"dependencies": [
        {"project_id": "needed", "dependency_type": "required"},
        {"project_id": "maybe", "dependency_type": "optional"},
        {"project_id": "clash", "dependency_type": "incompatible"},
        {"project_id": "bundled", "dependency_type": "embedded"},
    ]});
    let requirements = required("root", &document);
    assert_eq!(requirements.len(), 1);
    assert_eq!(requirements[0].project_id.as_deref(), Some("needed"));
    assert_eq!(requirements[0].version_id, None);
}

#[test]
fn a_dependency_on_the_root_project_is_not_a_requirement() {
    // The version is already being installed; this is the same
    // deduplication every other already-planned project gets.
    let document = json!({"dependencies": [{"project_id": "root", "dependency_type": "required"}]});
    assert!(required("root", &document).is_empty());
}

#[test]
fn an_explicit_pin_wins_even_over_compatibility() {
    let candidates = vec![
        version("lib", "fresh", &["1.20.1"], &["fabric"], "fresh.jar"),
        version("lib", "pinned", &["1.19.4"], &["fabric"], "pinned.jar"),
    ];
    let requirement = Requirement {
        project_id: Some("lib".into()),
        version_id: Some("pinned".into()),
    };
    let chosen = choose_version(&candidates, &requirement, "1.20.1", "fabric").unwrap();
    assert_eq!(chosen["id"], "pinned");
}

#[test]
fn a_pin_the_catalogue_does_not_list_is_an_error() {
    let candidates = vec![version(
        "lib",
        "fresh",
        &["1.20.1"],
        &["fabric"],
        "fresh.jar",
    )];
    let requirement = Requirement {
        project_id: Some("lib".into()),
        version_id: Some("missing".into()),
    };
    let error = choose_version(&candidates, &requirement, "1.20.1", "fabric").unwrap_err();
    assert!(error.to_string().contains("missing"), "{error}");
    assert!(error.to_string().contains("lib"), "{error}");
}

#[test]
fn the_first_compatible_candidate_is_chosen() {
    // The catalogue's own order is its relevance order; an incompatible
    // document before the compatible ones must not stop the walk.
    let candidates = vec![
        version("lib", "wrong-game", &["1.19.4"], &["fabric"], "old.jar"),
        version("lib", "wrong-loader", &["1.20.1"], &["forge"], "forge.jar"),
        version("lib", "first", &["1.20.1"], &["fabric"], "first.jar"),
        version("lib", "second", &["1.20.1"], &["fabric"], "second.jar"),
    ];
    let requirement = Requirement {
        project_id: Some("lib".into()),
        version_id: None,
    };
    let chosen = choose_version(&candidates, &requirement, "1.20.1", "fabric").unwrap();
    assert_eq!(chosen["id"], "first");
}

#[test]
fn no_compatible_candidate_is_an_error_naming_the_project() {
    let candidates = vec![version("lib", "old", &["1.19.4"], &["fabric"], "old.jar")];
    let requirement = Requirement {
        project_id: Some("lib".into()),
        version_id: None,
    };
    let error = choose_version(&candidates, &requirement, "1.20.1", "fabric").unwrap_err();
    assert!(error.to_string().contains("lib"), "{error}");
    assert!(error.to_string().contains("1.20.1"), "{error}");
}

#[test]
fn the_walk_reaches_a_dependencys_dependency() {
    let catalogue = catalogue(vec![
        (
            "A",
            vec![requiring(
                "A",
                "a1",
                json!([{"project_id": "L", "dependency_type": "required"}]),
            )],
        ),
        (
            "L",
            vec![version("L", "l1", &["1.20.1"], &["fabric"], "l.jar")],
        ),
    ]);
    let root = requiring(
        "root",
        "r1",
        json!([{"project_id": "A", "dependency_type": "required"}]),
    );
    let mut lookup = lookup(&catalogue);
    let plan = resolve_plan(&root, &mut lookup, "1.20.1", "fabric").unwrap();
    let ids: Vec<&str> = plan.iter().map(|file| file.project_id.as_str()).collect();
    assert_eq!(ids, vec!["A", "L"]);
    assert_eq!(plan[1].file_name, "l.jar");
    assert_eq!(
        plan[1].url.as_deref(),
        Some("https://cdn.example/l.jar"),
        "the plan carries what the fetch needs"
    );
    assert_eq!(plan[1].size, Some(7));
}

#[test]
fn a_shared_dependency_is_planned_once() {
    // A diamond is not a problem: two paths to one library is still one file.
    let catalogue = catalogue(vec![
        (
            "A",
            vec![requiring(
                "A",
                "a1",
                json!([{"project_id": "L", "dependency_type": "required"}]),
            )],
        ),
        (
            "B",
            vec![requiring(
                "B",
                "b1",
                json!([{"project_id": "L", "dependency_type": "required"}]),
            )],
        ),
        (
            "L",
            vec![version("L", "l1", &["1.20.1"], &["fabric"], "l.jar")],
        ),
    ]);
    let root = requiring(
        "root",
        "r1",
        json!([
            {"project_id": "A", "dependency_type": "required"},
            {"project_id": "B", "dependency_type": "required"},
        ]),
    );
    let mut lookup = lookup(&catalogue);
    let plan = resolve_plan(&root, &mut lookup, "1.20.1", "fabric").unwrap();
    let ids: Vec<&str> = plan.iter().map(|file| file.project_id.as_str()).collect();
    assert_eq!(ids, vec!["A", "B", "L"]);
}

#[test]
fn a_cycle_is_an_error_rather_than_a_walk_that_never_ends() {
    let catalogue = catalogue(vec![
        (
            "A",
            vec![requiring(
                "A",
                "a1",
                json!([{"project_id": "B", "dependency_type": "required"}]),
            )],
        ),
        (
            "B",
            vec![requiring(
                "B",
                "b1",
                json!([{"project_id": "A", "dependency_type": "required"}]),
            )],
        ),
    ]);
    let root = requiring(
        "root",
        "r1",
        json!([{"project_id": "A", "dependency_type": "required"}]),
    );
    let mut lookup = lookup(&catalogue);
    let error = resolve_plan(&root, &mut lookup, "1.20.1", "fabric").unwrap_err();
    assert!(error.to_string().contains("cycle"), "{error}");
    assert!(error.to_string().contains("A -> B -> A"), "{error}");
}

#[test]
fn a_chain_past_the_depth_cap_says_so() {
    let depth = MAX_DEPTH + 1;
    let mut catalogue = BTreeMap::new();
    for index in 0..=depth {
        let project = format!("d{index}");
        let next = format!("d{}", index + 1);
        catalogue.insert(
            project.clone(),
            vec![requiring(
                &project,
                &format!("v{index}"),
                json!([{"project_id": next, "dependency_type": "required"}]),
            )],
        );
    }
    let root = requiring(
        "root",
        "r1",
        json!([{"project_id": "d0", "dependency_type": "required"}]),
    );
    let mut lookup = lookup(&catalogue);
    let error = resolve_plan(&root, &mut lookup, "1.20.1", "fabric").unwrap_err();
    assert!(error.to_string().contains("deep"), "{error}");
    assert!(error.to_string().contains("d8"), "{error}");
}

#[test]
fn the_plan_is_the_same_every_run() {
    let catalogue = catalogue(vec![
        (
            "A",
            vec![requiring(
                "A",
                "a1",
                json!([{"project_id": "L", "dependency_type": "required"}]),
            )],
        ),
        (
            "B",
            vec![requiring(
                "B",
                "b1",
                json!([{"project_id": "L", "dependency_type": "required"}]),
            )],
        ),
        (
            "L",
            vec![version("L", "l1", &["1.20.1"], &["fabric"], "l.jar")],
        ),
    ]);
    let root = requiring(
        "root",
        "r1",
        json!([
            {"project_id": "A", "dependency_type": "required"},
            {"project_id": "B", "dependency_type": "required"},
        ]),
    );
    let mut first = lookup(&catalogue);
    let mut second = lookup(&catalogue);
    let once = resolve_plan(&root, &mut first, "1.20.1", "fabric").unwrap();
    let twice = resolve_plan(&root, &mut second, "1.20.1", "fabric").unwrap();
    assert_eq!(once, twice);
}

#[test]
fn a_dependency_that_names_only_a_version_is_looked_up_by_it() {
    // The one lookup answers either id, so a version-pinned dependency
    // resolves without a second entry point.
    let catalogue = catalogue(vec![(
        "v9",
        vec![version("lib", "v9", &["1.20.1"], &["fabric"], "nine.jar")],
    )]);
    let root = requiring(
        "root",
        "r1",
        json!([{"version_id": "v9", "dependency_type": "required"}]),
    );
    let mut lookup = lookup(&catalogue);
    let plan = resolve_plan(&root, &mut lookup, "1.20.1", "fabric").unwrap();
    assert_eq!(plan.len(), 1);
    assert_eq!(plan[0].version_id, "v9");
    assert_eq!(plan[0].project_id, "lib", "the chosen document names it");
    assert_eq!(plan[0].file_name, "nine.jar");
}

#[test]
fn a_required_dependency_naming_nothing_is_an_error() {
    let catalogue = catalogue(vec![]);
    let root = requiring("root", "r1", json!([{"dependency_type": "required"}]));
    let mut lookup = lookup(&catalogue);
    let error = resolve_plan(&root, &mut lookup, "1.20.1", "fabric").unwrap_err();
    assert!(
        error
            .to_string()
            .contains("neither a project nor a version"),
        "{error}"
    );
}

#[test]
fn optional_files_are_planned_only_when_selected() {
    let files = json!([
        entry("mods/required.jar"),
        json!({
            "path": "mods/optional.jar",
            "downloads": ["https://cdn.example/o.jar"],
            "hashes": {"sha1": "cc"},
            "env": {"client": "optional"}
        })
    ]);
    let without = plan_files(&index(files.clone()), &[]).unwrap();
    assert_eq!(without.files.len(), 1);
    assert_eq!(planned_path(&without.files[0]), "mods/required.jar");

    let with = plan_files(&index(files), &["mods/optional.jar".to_string()]).unwrap();
    assert_eq!(with.files.len(), 2);
    assert_eq!(planned_path(&with.files[1]), "mods/optional.jar");
}

#[test]
fn a_file_the_client_cannot_use_is_not_planned() {
    let files = json!([json!({
        "path": "server-only.jar",
        "downloads": ["https://cdn.example/s.jar"],
        "hashes": {"sha1": "dd"},
        "env": {"client": "unsupported", "server": "required"}
    })]);
    assert!(plan_files(&index(files), &[]).unwrap().files.is_empty());
}

#[test]
fn a_file_with_no_download_is_planned_as_local() {
    let files = json!([json!({"path": "mods/handcrafted.jar", "downloads": [], "hashes": {}})]);
    let plan = plan_files(&index(files), &[]).unwrap();
    assert!(matches!(&plan.files[0], SelectedFile::Local { .. }));
    assert_eq!(planned_path(&plan.files[0]), "mods/handcrafted.jar");
}

#[test]
fn the_first_download_url_is_the_one_planned() {
    let files = json!([json!({
        "path": "mods/a.jar",
        "downloads": ["https://first.example/a.jar", "https://second.example/a.jar"],
        "hashes": {"sha1": "ee"}
    })]);
    let plan = plan_files(&index(files), &[]).unwrap();
    match &plan.files[0] {
        SelectedFile::Download { url, size, .. } => {
            assert_eq!(url, "https://first.example/a.jar");
            assert_eq!(*size, None, "an absent fileSize stays absent");
        }
        other => panic!("expected a download, got {other:?}"),
    }
}

#[test]
fn a_path_that_escapes_the_game_directory_is_refused() {
    for path in [
        "../evil.jar",
        "/etc/passwd",
        "mods\\evil.jar",
        "C:/evil.jar",
        "mods//evil.jar",
        "",
    ] {
        let error = plan_files(&index(json!([entry(path)])), &[]).unwrap_err();
        assert!(
            error.to_string().contains("escape"),
            "{path} was not refused: {error}"
        );
    }
}

#[test]
fn the_client_layer_overwrites_the_base_one() {
    let layers = override_layers();
    let sources: Vec<&str> = layers.iter().map(|layer| layer.source.as_str()).collect();
    assert_eq!(sources, vec!["overrides", "client-overrides"]);
    assert!(
        layers.iter().all(|layer| layer.destination.is_empty()),
        "a layer keeps the pack's own relative paths"
    );
    let plan = plan_files(&index(json!([])), &[]).unwrap();
    assert_eq!(plan.overrides, layers);
}

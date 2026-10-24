//! `project.rusty` (#853, #1319): its fields, and recording the engine on an
//! edit-open.

use super::*;
use crate::core::application::{WindowMode, DEFAULT_STARTUP_SCENE};
use crate::core::project::{pin_engine, Verdict};

fn fresh(name: &str) -> std::path::PathBuf {
    let dir = crate::test_temp::dir().join("project_file").join(name);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const A_DIRTY: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa-dirty";

/// A project under `name` that recorded engine `A` at `order`; its file's path.
fn opened_by_a(name: &str, order: Option<u64>) -> (std::path::PathBuf, ProjectFile) {
    let root = fresh(name);
    let file = ProjectFile {
        engine_commit: A.to_string(),
        engine_order: order,
        ..ProjectFile::default()
    };
    file.write(&root.join(PROJECT_FILE)).unwrap();
    (root, file)
}

#[test]
fn a_partial_file_keeps_the_defaults() {
    let file = ProjectFile::from_json(r#"{ "build": { "product_name": "Neon" } }"#).unwrap();
    assert_eq!(file.build.product_name, "Neon");
    assert_eq!(file.build.startup_scene, DEFAULT_STARTUP_SCENE);
    assert_eq!(file.build.window_mode, WindowMode::Windowed);
    assert_eq!(file.engine_commit, engine_commit());
    assert!(ProjectFile::from_json("{ not json").is_err());
}

#[test]
fn every_build_field_roundtrips() {
    let file = ProjectFile {
        build: BuildSettings {
            startup_scene: "assets/scenes/menu.scene".into(),
            product_name: "Neon".into(),
            window_mode: WindowMode::Fullscreen,
        },
        ..ProjectFile::default()
    };
    assert_eq!(ProjectFile::from_json(&file.to_json()).unwrap(), file);
}

#[test]
fn a_new_project_gets_a_default_file_named_after_its_folder() {
    let root = fresh("Horde");
    let (engine, lines) = record(&root).unwrap();
    assert_eq!(engine.verdict(), Verdict::Same);
    assert_eq!(lines, vec![format!("created {PROJECT_FILE}")]);
    let file = ProjectFile::read(&root.join(PROJECT_FILE))
        .unwrap()
        .unwrap();
    let mut expected = ProjectFile::default();
    expected.build.product_name = "Horde".to_string();
    assert_eq!(file, expected);
}

#[test]
fn an_upgrade_records_the_new_engine_and_says_so() {
    let (root, stale) = opened_by_a("upgrade", Some(5));
    pin_engine(B, Some(6));
    assert_eq!(inspect(&root).unwrap().verdict(), Verdict::Upgrade);
    let path = root.join(PROJECT_FILE);
    assert_eq!(
        ProjectFile::read(&path).unwrap(),
        Some(stale.clone()),
        "run: untouched"
    );

    let (engine, lines) = record(&root).unwrap();
    assert_eq!(
        engine.recorded.as_deref(),
        Some(A),
        "the check sees the old record"
    );
    assert_eq!(engine.warning(), None);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(
        lines[0].starts_with("upgraded from engine aaaa"),
        "{lines:?}"
    );
    let now = ProjectFile::read(&path).unwrap().unwrap();
    assert_eq!((now.engine_commit.as_str(), now.engine_order), (B, Some(6)));
    assert_eq!(
        now.build, stale.build,
        "an existing file keeps its product name"
    );
}

#[test]
fn a_file_without_a_count_is_an_upgrade() {
    let (root, _) = opened_by_a("countless", None);
    pin_engine(B, Some(6));
    assert_eq!(inspect(&root).unwrap().verdict(), Verdict::Upgrade);
    let text = std::fs::read_to_string(root.join(PROJECT_FILE)).unwrap();
    assert!(!text.contains("engine_order"), "no count, no field: {text}");
}

#[test]
fn an_older_engine_still_warns_and_records_itself() {
    let (root, _) = opened_by_a("older", Some(7));
    pin_engine(B, Some(6));
    let (engine, lines) = record(&root).unwrap();
    assert!(engine.warning().is_some() && lines.is_empty(), "{lines:?}");
    let now = ProjectFile::load(&root).unwrap();
    assert_eq!((now.engine_commit.as_str(), now.engine_order), (B, Some(6)));
}

#[test]
fn a_build_that_cannot_count_or_name_itself_keeps_what_it_can() {
    let (root, _) = opened_by_a("shallow", Some(5));
    pin_engine(A_DIRTY, None);
    record(&root).unwrap();
    let now = ProjectFile::load(&root).unwrap();
    assert_eq!(now.engine_order, Some(5), "same commit: the count holds");

    pin_engine(UNKNOWN_COMMIT, None);
    let (engine, lines) = record(&root).unwrap();
    assert_eq!((engine.warning(), lines), (None, vec![]));
    assert_eq!(
        ProjectFile::load(&root).unwrap(),
        now,
        "the last known record"
    );
}

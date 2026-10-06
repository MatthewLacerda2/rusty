//! `project.rusty` (#853): folding the legacy settings in, recording the engine, and
//! the check a frontend shows.

use super::*;

fn fresh(name: &str) -> std::path::PathBuf {
    let dir = crate::test_temp::dir().join("project_file").join(name);
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn check(recorded: Option<&str>, running: &str) -> EngineCheck {
    EngineCheck {
        recorded: recorded.map(str::to_string),
        running: running.to_string(),
    }
}

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

#[test]
fn only_two_known_different_commits_mismatch() {
    assert!(check(Some(A), A).matches());
    assert!(check(None, A).matches(), "nothing recorded yet");
    assert!(
        check(Some(&format!("{A}-dirty")), A).matches(),
        "dirty is ignored"
    );
    assert!(check(Some(UNKNOWN_COMMIT), A).matches());
    assert!(check(Some(A), UNKNOWN_COMMIT).matches());
    assert!(!check(Some(A), B).matches());
    assert_eq!(check(Some(A), A).warning(), None);
    let line = check(Some(A), &format!("{B}-dirty")).warning().unwrap();
    assert!(line.contains("aaaaaaaaaaaa,"), "{line}");
    assert!(line.contains("bbbbbbbbbbbb-dirty:"), "{line}");
    assert!(!line.contains(A), "commits are shortened: {line}");
}

#[test]
fn an_empty_record_is_no_record() {
    assert_eq!(EngineCheck::against(Some("")).recorded, None);
    assert_eq!(EngineCheck::against(Some(A)).recorded.as_deref(), Some(A));
    assert_eq!(EngineCheck::against(None).running, ENGINE_COMMIT);
}

#[test]
fn a_partial_file_keeps_the_defaults() {
    let file = ProjectFile::from_json(r#"{ "build": { "product_name": "Neon" } }"#).unwrap();
    assert_eq!(file.build.product_name, "Neon");
    assert_eq!(file.engine_commit, ENGINE_COMMIT);
    assert_eq!(ProjectFile::from_json(&file.to_json()).unwrap(), file);
    assert!(ProjectFile::from_json("{ not json").is_err());
}

#[test]
fn opening_folds_the_legacy_settings_in_once() {
    let root = fresh("fold");
    let legacy = root.join(LEGACY_BUILD_SETTINGS);
    std::fs::write(&legacy, r#"{ "product_name": "Neon" }"#).unwrap();

    assert_eq!(
        inspect(&root).unwrap().recorded,
        None,
        "pre-#853: no engine"
    );
    assert!(!root.join(PROJECT_FILE).exists(), "a run writes nothing");
    assert_eq!(ProjectFile::load(&root).unwrap().build.product_name, "Neon");

    let (engine, lines) = record(&root).unwrap();
    assert_eq!(engine.recorded, None);
    assert!(lines.iter().any(|l| l.contains("folded")), "{lines:?}");
    assert!(!legacy.exists());
    let file = ProjectFile::read(&root.join(PROJECT_FILE))
        .unwrap()
        .unwrap();
    assert_eq!(file.build.product_name, "Neon");
    assert_eq!(file.engine_commit, ENGINE_COMMIT);

    let (again, lines) = record(&root).unwrap();
    assert!(lines.is_empty(), "{lines:?}");
    assert_eq!(again.recorded.as_deref(), Some(ENGINE_COMMIT));
    assert!(again.matches());
}

#[test]
fn a_new_project_gets_a_default_file() {
    let root = fresh("new");
    let (engine, lines) = record(&root).unwrap();
    assert!(engine.matches());
    assert_eq!(lines, vec![format!("created {PROJECT_FILE}")]);
    let file = ProjectFile::read(&root.join(PROJECT_FILE))
        .unwrap()
        .unwrap();
    assert_eq!(file, ProjectFile::default());
}

#[test]
fn opening_for_edit_records_the_running_engine() {
    let root = fresh("stale");
    let stale = ProjectFile {
        engine_commit: A.to_string(),
        ..ProjectFile::default()
    };
    let path = root.join(PROJECT_FILE);
    stale.write(&path).unwrap();
    assert_eq!(inspect(&root).unwrap().recorded.as_deref(), Some(A));
    assert_eq!(
        ProjectFile::read(&path).unwrap().unwrap(),
        stale,
        "run: untouched"
    );

    let (engine, _) = record(&root).unwrap();
    assert_eq!(
        engine.recorded.as_deref(),
        Some(A),
        "the check sees the old record"
    );
    let now = ProjectFile::read(&path).unwrap().unwrap().engine_commit;
    let expected = if ENGINE_COMMIT == UNKNOWN_COMMIT {
        A
    } else {
        ENGINE_COMMIT
    };
    assert_eq!(now, expected);
}

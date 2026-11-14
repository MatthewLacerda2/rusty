//! The bake reports (#1980): written beside the scene with the common block every
//! one opens with, read back, and each kind's headline signal on a primitive scene.

mod navmesh;
mod occlusion;
mod probes;
mod reflections;

use serde_json::json;

use super::{keep, read, report_path, Kind, Report, Stages};

/// A fresh temp dir and a scene path inside it.
pub(super) fn scene_path(name: &str) -> String {
    let dir = rusty_test_support::temp::dir().join(format!("rusty_1980_{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir.join("level.scene").to_string_lossy().into_owned()
}

/// The keys every report opens with.
pub(super) const COMMON: [&str; 9] = [
    "kind",
    "engine",
    "timestamp",
    "threads",
    "wallSeconds",
    "stages",
    "settings",
    "inputFingerprint",
    "outputBytes",
];

/// Assert `report` is a `kind` report carrying the common block.
pub(super) fn assert_common(report: &serde_json::Value, kind: Kind) {
    for key in COMMON {
        assert!(report.get(key).is_some(), "{key} missing from {report}");
    }
    assert_eq!(report["kind"], kind.name());
}

#[test]
fn a_report_is_written_beside_the_scene_and_read_back() {
    let path = scene_path("roundtrip");
    let mut stages = Stages::start();
    stages.push("bake", std::time::Duration::from_millis(1500));
    stages.push("write", std::time::Duration::from_millis(250));
    let report = Report::new(Kind::Navmesh, &stages, json!({ "a": 1 }), 0xabc, 42);
    let file = report.with(json!({ "spans": 7 })).write(&path).unwrap();
    assert_eq!(file, report_path(&path, Kind::Navmesh));
    assert!(file.ends_with("level.scene.bake/navmesh.json"), "{file:?}");
    let back = read(&path, Kind::Navmesh).expect("a report");
    assert_common(&back, Kind::Navmesh);
    assert_eq!(back["spans"], 7);
    assert_eq!(back["inputFingerprint"], "0000000000000abc");
    assert_eq!(
        (back["outputBytes"].as_u64(), back["wallSeconds"].as_f64()),
        (Some(42), Some(1.75))
    );
    assert_eq!(
        back["stages"][1],
        json!({ "name": "write", "seconds": 0.25 })
    );
    assert_eq!(back["settings"]["a"], 1);
    assert!(
        read(&path, Kind::Occlusion).is_none(),
        "another kind never baked"
    );
}

#[test]
fn every_kind_names_its_own_file() {
    for kind in Kind::ALL {
        assert_eq!(Kind::parse(kind.name()), Some(kind));
    }
    assert_eq!(Kind::parse("navigation"), None);
}

#[test]
fn a_report_that_cannot_be_written_is_only_warned() {
    let path = scene_path("blocked");
    // A file where the folder should be: the write fails, the bake does not.
    std::fs::write(format!("{path}.bake"), b"in the way").unwrap();
    keep(&path, Report::new(Kind::Probes, &Stages::start(), (), 0, 0));
    assert!(read(&path, Kind::Probes).is_none());
}

#[test]
fn a_malformed_report_reads_as_none() {
    let path = scene_path("malformed");
    let file = report_path(&path, Kind::Lightmaps);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, b"{ \"kind\": \"lightm").unwrap();
    assert!(read(&path, Kind::Lightmaps).is_none());
}

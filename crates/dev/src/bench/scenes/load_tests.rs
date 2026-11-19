//! A scene's load in steps that tile it, its repeats folded into the warm loads'
//! spread with the cold one apart, and counts of what it did.

use glam::Vec3;
use rusty_scene::Scene;

use super::*;

fn of(steps: &[(&str, f64)], parsed: f64) -> Load {
    let steps = steps.iter().map(|(s, ms)| (s.to_string(), *ms)).collect();
    let counts = vec![("models_parsed".to_string(), parsed)];
    Load { steps, counts }
}

fn converting(converted: f64) -> Load {
    let mut load = of(&[("document", 1.0)], 1.0);
    load.counts.push((CONVERTED.to_string(), converted));
    load
}

#[test]
fn the_conversions_are_the_runs_sum_the_other_counts_the_last_loads() {
    let rows = rows(&[converting(2.0), converting(0.0), converting(0.0)]);
    let at = |m: &str| rows.iter().find(|r| r.metric == m).unwrap().avg;
    assert_eq!(
        at("load.count.models_converted"),
        2.0,
        "the first load converted"
    );
    assert_eq!(at("load.count.models_parsed"), 1.0);
}

#[test]
fn the_warm_loads_give_the_rows_and_their_spread_the_cold_one_apart() {
    let loads = [
        of(&[("document", 30.0), ("frame", 10.0)], 1.0),
        of(&[("document", 10.0), ("frame", 4.0)], 1.0),
        of(&[("document", 12.0), ("frame", 6.0)], 1.0),
    ];
    let rows = rows(&loads);
    let names: Vec<&str> = rows.iter().map(|r| r.metric.as_str()).collect();
    let want = ["load_ms", "load_cold_ms", "load.document", "load.frame"];
    assert_eq!(names[..4], want);
    assert_eq!(names[4], "load.count.models_parsed");
    let at = |m: &str| rows.iter().find(|r| r.metric == m).unwrap();
    assert_eq!(
        (at("load_ms").avg, at("load_ms").lo, at("load_ms").hi),
        (16.0, 14.0, 18.0)
    );
    assert_eq!(
        at("load_cold_ms").avg,
        40.0,
        "the first load, in no average"
    );
    assert_eq!(
        (at("load.document").lo, at("load.document").hi),
        (10.0, 12.0)
    );
    assert_eq!(at("load.count.models_parsed").avg, 1.0);
}

#[test]
fn one_load_stands_as_load_ms_with_no_cold_row() {
    let rows = rows(&[of(&[("document", 5.0), ("frame", 2.0)], 0.0)]);
    assert_eq!(rows[0].metric, "load_ms");
    assert_eq!((rows[0].avg, rows[0].lo, rows[0].hi), (7.0, 7.0, 7.0));
    assert!(rows.iter().all(|r| r.metric != "load_cold_ms"));
    assert!(super::rows(&[]).is_empty());
}

/// A project of one scene holding a model instanced twice, the file forgotten by
/// this thread's import cache so the load parses it.
fn project_with_a_model() -> std::path::PathBuf {
    let root = rusty_test_support::temp::dir().join(format!("load_rows_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("assets/scenes")).unwrap();
    let model = rusty_test_support::models::quad_file("load_rows_quad");
    let mut scene = Scene::new();
    let reference = format!("{model}::Quad");
    for _ in 0..2 {
        rusty_edit::asset_instance::instantiate_asset(&mut scene, &reference, None, Vec3::ZERO)
            .unwrap();
    }
    let path = root.join("assets/scenes/quad.scene");
    rusty_edit::save_to_file(&scene, &path.to_string_lossy()).unwrap();
    rusty_asset::import_cache::forget(std::path::Path::new(&model));
    root
}

#[test]
fn gpu_a_load_runs_every_step_once_in_order_and_they_sum_to_load_ms() {
    // Alone: `converted` counts the process's conversions, any test's.
    rusty_test_support::alone!(a_load_in_steps);
}

fn a_load_in_steps() {
    let root = project_with_a_model();
    let opts = Options {
        size: (32, 32),
        warm: false,
        ..Options::default()
    };
    let settings = rusty_core::project::ProjectSettings::default();
    let (harness, load) = load(&root.join("out"), &root, "quad", settings, opts).unwrap();
    let names: Vec<&str> = load.steps.iter().map(|(s, _)| s.as_str()).collect();
    let want = [
        "document", "import", "apply", "navmesh", "world", "refresh", "scripts", "physics",
        "startup", "tick", "frame",
    ];
    assert_eq!(names, want);
    let steps = &load.steps;
    assert!(steps.iter().all(|(_, ms)| *ms >= 0.0), "{steps:?}");
    assert!(load.total_ms() > 0.0);
    assert_eq!(
        harness.frame(),
        1,
        "one tick: the steps ran no extra sim step"
    );
    let counts = load.counts.clone();
    assert_eq!(
        counts[0],
        ("models_parsed".to_string(), 1.0),
        "one file, parsed once"
    );
    assert_eq!(
        counts[1],
        (CONVERTED.to_string(), 0.0),
        "a glTF converts nothing"
    );
    tiles(&load, &want);
    // Rendered (an adapter was there): the texture and upload counts follow.
    if counts.len() > 2 {
        let names: Vec<&str> = counts.iter().map(|(c, _)| c.as_str()).collect();
        assert_eq!(
            names[2..],
            ["textures_compressed", "textures_decoded", "upload_mb"]
        );
    }
}

/// The load's rows: its steps' rows sum to its `load_ms` (nothing timed twice or
/// left out).
fn tiles(load: &Load, steps: &[&str]) {
    let rows = rows(std::slice::from_ref(load));
    let row = |m: &str| rows.iter().find(|r| r.metric == m).map(|r| r.avg);
    let sum: f64 = steps.iter().filter_map(|s| row(&format!("load.{s}"))).sum();
    let total = row("load_ms").unwrap();
    assert!((total - sum).abs() < 1.0, "the steps tile the load");
}

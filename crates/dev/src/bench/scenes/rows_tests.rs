//! The time rows' parts hold their parent whole: every system the schedule runs
//! is a `cpu.<system>` row (named by the schedule, so a fixture system of known
//! cost shows under its own name), every renderer stage a `render.<stage>` row,
//! and what no part holds (`cpu.other`, `render.other`) stays within timer
//! overhead. Renders when an adapter is there (hence `gpu_`); without one the
//! render half is skipped.

use std::time::Duration;

use rusty_app::{Resources, Stage, World};
use rusty_components::CameraComponent;
use rusty_scene::Scene;

use super::super::{measure, Report};
use super::*;

/// The fixture system's cost per tick.
const NAP: Duration = Duration::from_millis(4);

fn napper(_: &mut World, _: &mut Resources) {
    std::thread::sleep(NAP);
}

fn avg(report: &Report, metric: &str) -> Option<f64> {
    let row = report.metrics.iter().find(|(m, _)| m == metric);
    row.map(|(_, s)| s.avg)
}

/// The sum of `prefix.*` rows, `other` included, and the `other` row alone.
fn parts(report: &Report, prefix: &str) -> (f64, f64) {
    let rows = report.metrics.iter().filter(|(m, _)| m.starts_with(prefix));
    let sum = rows.map(|(_, s)| s.avg).sum();
    (
        sum,
        avg(report, &format!("{prefix}other")).unwrap_or(f64::NAN),
    )
}

#[test]
fn gpu_every_system_and_render_stage_is_a_row_and_together_they_are_the_whole() {
    let root = rusty_test_support::temp::dir().join(format!("rows_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("assets/scenes")).unwrap();
    let mut scene = Scene::new();
    let cam = scene.add_entity("Cam".into());
    scene
        .world
        .set_camera(cam, Some(CameraComponent::default()));
    let path = root.join("assets/scenes/a.scene");
    rusty_edit::save_to_file(&scene, &path.to_string_lossy()).unwrap();
    let settings = ProjectSettings::default();
    let (mut harness, _) = load(&root.join("out"), &root, "a", settings, (64, 64)).unwrap();
    let mut world = harness.world.borrow_mut();
    world
        .resources
        .schedule_mut()
        .add_system(Stage::FixedUpdate, napper);
    drop(world);

    let report = measure(&mut harness, 6);
    let napped = avg(&report, "cpu.napper").expect("the fixture system has its row");
    let cpu = avg(&report, "cpu_ms").unwrap();
    // Scaled by the same CPU clock as cpu_ms: its share is what is measured.
    assert!(
        napped / cpu > 0.5,
        "the nap is most of the tick: {napped} of {cpu}"
    );
    assert!(
        avg(&report, "cpu.update_scripts").is_some(),
        "{:?}",
        report.metrics
    );
    let (sum, other) = parts(&report, "cpu.");
    assert!(
        (sum - cpu).abs() <= cpu * 1e-9,
        "cpu rows {sum} sum to cpu_ms {cpu}"
    );
    assert!(other.abs() <= cpu * 0.03, "cpu.other {other} of {cpu}");

    let Some(renderer) = avg(&report, "renderer_ms") else {
        return; // no adapter: nothing rendered
    };
    for stage in [
        "render.extract",
        "render.shadows",
        "render.shadow_gather",
        "render.scene",
    ] {
        assert!(
            avg(&report, stage).is_some(),
            "{stage}: {:?}",
            report.metrics
        );
    }
    let (sum, other) = parts(&report, "render.");
    assert!(
        (sum - renderer).abs() <= renderer * 1e-9,
        "{sum} vs {renderer}"
    );
    assert!(
        other.abs() <= renderer * 0.03,
        "render.other {other} of {renderer}"
    );
}

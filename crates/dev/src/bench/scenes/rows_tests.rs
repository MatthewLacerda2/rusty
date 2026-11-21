use std::time::Duration;

use rusty_app::{Resources, Stage, World};
use rusty_components::{CameraComponent, LightComponent, LightType};
use rusty_scene::Scene;

use super::super::{measure, Report};
use super::*;

const NAP: Duration = Duration::from_millis(4);

fn napper(_: &mut World, _: &mut Resources) {
    std::thread::sleep(NAP);
}

fn avg(report: &Report, metric: &str) -> Option<f64> {
    let row = report.metrics.iter().find(|(m, _)| m == metric);
    row.map(|(_, s)| s.avg)
}

fn parts(report: &Report, prefix: &str) -> (f64, f64) {
    let part = |m: &str| m.strip_prefix(prefix).is_some_and(|p| !p.contains('.'));
    let rows = report.metrics.iter().filter(|(m, _)| part(m));
    let sum = rows.map(|(_, s)| s.avg).sum();
    (
        sum,
        avg(report, &format!("{prefix}other")).unwrap_or(f64::NAN),
    )
}

fn measured() -> Report {
    let root = rusty_test_support::temp::dir().join(format!("rows_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(root.join("assets/scenes")).unwrap();
    let mut scene = Scene::new();
    let cam = scene.add_entity("Cam".into());
    let camera = Some(CameraComponent::default());
    scene.world.set_camera(cam, camera);
    let sun = scene.add_entity("Sun".into());
    let light = LightComponent {
        light_type: LightType::Directional,
        cast_shadows: true,
        mode: rusty_components::LightMode::Mixed,
        ..rusty_edit::authoring::default_light()
    };
    scene.world.set_light(sun, Some(light));
    let path = root.join("assets/scenes/a.scene");
    rusty_edit::save_to_file(&scene, &path.to_string_lossy()).unwrap();
    let settings = ProjectSettings::default();
    let opts = Options {
        size: (64, 64),
        ..Options::default()
    };
    let (mut harness, _) = load(&root.join("out"), &root, "a", settings, opts).unwrap();
    let mut world = harness.world.borrow_mut();
    let schedule = world.resources.schedule_mut();
    schedule.add_system(Stage::FixedUpdate, napper);
    drop(world);
    measure(&mut harness, 6)
}

fn holds_whole(report: &Report, prefix: &str, total: f64, bound: f64) {
    let (sum, other) = parts(report, prefix);
    assert!(
        (sum - total).abs() <= total * 1e-9,
        "{prefix} rows {sum} vs {total}"
    );
    assert!(
        (-total * 1e-9..=bound).contains(&other),
        "{prefix}other {other} of {total}, above {bound}"
    );
}

#[test]
fn gpu_every_system_and_render_stage_is_a_row_and_together_they_are_the_whole() {
    let report = measured();
    let napped = avg(&report, "cpu.napper").expect("the fixture system has its row");
    let cpu = avg(&report, "cpu_ms").unwrap();
    assert!(
        napped / cpu > 0.5,
        "the nap is most of the tick: {napped} of {cpu}"
    );
    assert!(
        avg(&report, "cpu.flush_bounds").is_some(),
        "the flush has its row"
    );
    let scripts = avg(&report, "cpu.update_scripts");
    assert!(scripts.is_some(), "{:?}", report.metrics);
    holds_whole(&report, "cpu.", cpu, 0.75 * napped);

    let Some(renderer) = avg(&report, "renderer_ms") else {
        return;
    };
    for stage in ["extract", "shadows", "shadow_gather", "scene"] {
        let row = avg(&report, &format!("render.{stage}"));
        assert!(row.is_some(), "{stage}: {:?}", report.metrics);
    }
    holds_whole(&report, "render.", renderer, renderer * 0.03);
}

#[test]
fn resident_bytes_by_kind_are_rows_in_mib() {
    let counters = rusty_render::RenderCounters {
        texture_bytes: 3 << 20,
        lightmap_bytes: 1 << 19,
        mesh_bytes: 2 << 20,
        ..Default::default()
    };
    let mut report = SceneReport::default();
    push_resident(&mut report, &counters);
    let mb = |k: &str| report.row(k).map(|r| r.avg);
    assert_eq!(mb("gpu_texture_mb"), Some(3.0));
    assert_eq!(mb("gpu_lightmap_mb"), Some(0.5));
    assert_eq!(mb("gpu_mesh_mb"), Some(2.0));
    assert_eq!(
        mb("gpu_reflection_mb"),
        Some(0.0),
        "a kind the scene lacks reads 0"
    );
}

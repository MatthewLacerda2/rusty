//! `--set` end to end (#2381): the GPU occlusion cull, off in every seeded
//! preset, tests nothing on a primitive scene's bench, and tests its meshes with
//! `gpu_occlusion_culling=true` set for the run; the report records the
//! override, and the project's `project.rusty` is byte-identical after it.

use glam::Vec3;
use rusty_components::CameraComponent;
use rusty_navigation::test_support::add_floor;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::Scene;

use super::set::Overrides;
use super::*;

const OPTS: Options = Options {
    size: (64, 64),
    warmup: 1,
    frames: 2,
    runs: 1,
    warm: false,
};

/// A walled yard and its project file, written to a temp dir.
fn project() -> PathBuf {
    let root = rusty_test_support::temp::dir().join(format!("set_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    let scenes = root.join("assets/scenes");
    std::fs::create_dir_all(&scenes).unwrap();
    let mut yard = Scene::new();
    add_floor(&mut yard, -5.0, 5.0, -5.0, 5.0);
    for (name, at, size) in [
        ("Wall", Vec3::new(0.0, 1.5, 1.0), Vec3::new(4.0, 3.0, 0.2)),
        ("Hidden", Vec3::new(0.0, 0.5, 3.0), Vec3::ONE),
    ] {
        let id = create_entity(&mut yard, name, Some(Primitive::Box));
        let mut t = yard.world.transform_mut(id).unwrap();
        (t.position, t.scale) = (at, size);
    }
    // Crates before the wall: the cull skips a camera drawing fewer than
    // `MIN_INSTANCES` (32) opaque instances (#2338).
    for k in 0..32 {
        let id = create_entity(&mut yard, "Crate", Some(Primitive::Box));
        let mut t = yard.world.transform_mut(id).unwrap();
        let at = Vec3::new((k % 8) as f32 * 0.5 - 1.75, 0.3 + (k / 8) as f32 * 0.5, 0.5);
        (t.position, t.scale) = (at, Vec3::splat(0.3));
    }
    let cam = yard.add_entity("Cam".into());
    yard.world.set_camera(cam, Some(CameraComponent::default()));
    yard.world
        .set_world_position(cam, Vec3::new(0.0, 1.5, -4.0));
    let path = scenes.join("yard.scene");
    rusty_edit::save_to_file(&yard, &path.to_string_lossy()).unwrap();
    let file = serde_json::to_string_pretty(&rusty_core::project::ProjectFile::default());
    std::fs::write(root.join(rusty_core::project::PROJECT_FILE), file.unwrap()).unwrap();
    root
}

fn tested(report: &SceneReport) -> Option<f64> {
    report.row("hiz_tested").map(|r| r.avg)
}

#[test]
fn gpu_set_turns_the_occlusion_cull_on_for_one_run_and_writes_nothing() {
    let root = project();
    let file = root.join(rusty_core::project::PROJECT_FILE);
    let before = std::fs::read(&file).unwrap();
    let out = root.join("out");
    let settings = || {
        let text = std::fs::read_to_string(&file).unwrap();
        ProjectSettings::from(rusty_core::project::ProjectFile::from_json(&text).unwrap())
    };
    let off = bench(&root, "yard", settings(), &out, OPTS, &Extra::default()).unwrap();
    assert!(off.set.is_empty());
    let on_set = Overrides::parse(&["gpu_occlusion_culling=true".to_string()]).unwrap();
    let mut on_settings = settings();
    on_set.apply(&mut on_settings.quality).unwrap();
    let extra = Extra {
        set: on_set.said.clone(),
        ..Extra::default()
    };
    let on = bench(&root, "yard", on_settings, &out, OPTS, &extra).unwrap();
    assert_eq!(on.set, ["gpu_occlusion_culling=true"]);
    assert!(
        on.warnings
            .iter()
            .any(|w| w.contains("other quality overrides")),
        "{:?}",
        on.warnings
    );
    assert_eq!(
        std::fs::read(&file).unwrap(),
        before,
        "project.rusty untouched"
    );
    if on.row("renderer_ms").is_none() {
        return; // no adapter: nothing rendered, nothing culled
    }
    assert_eq!(
        tested(&off),
        Some(0.0),
        "off in the seeded preset: {:?}",
        off.rows
    );
    assert!(tested(&on).is_some_and(|n| n > 0.0), "{:?}", on.rows);
}

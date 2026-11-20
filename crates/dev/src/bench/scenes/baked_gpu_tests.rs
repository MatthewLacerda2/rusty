//! #2481: a scene benched without the baked data it has on disk says so, one line a
//! kind: a `.occlusion` its load refused, a PVS its statics moved away from, a
//! navmesh Play's start refused; a scene whose bakes are current says nothing.

use glam::Vec3;
use rusty_components::CameraComponent;
use rusty_lighting::occlusion::bake_occlusion;
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::lighting::occlusion::{encode, occlusion_path};
use rusty_scene::Scene;

use super::*;

const OPTS: Options = Options {
    size: (32, 32),
    warmup: 1,
    frames: 1,
    runs: 1,
    warm: false,
};

/// A static wall between a camera and a box, and its PVS's bytes; the wall at `wall_x`.
fn walled(wall_x: f32) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    let wall = create_entity(&mut scene, "Wall", Some(Primitive::Box));
    let behind = create_entity(&mut scene, "Behind", Some(Primitive::Box));
    for (id, at, scale) in [
        (wall, Vec3::new(wall_x, 1.0, 0.0), Vec3::new(8.0, 4.0, 0.5)),
        (behind, Vec3::new(0.0, 0.5, 4.0), Vec3::ONE),
    ] {
        scene.world.set_static(id, true);
        let mut t = scene.world.transform_mut(id).unwrap();
        (t.position, t.scale) = (at, scale);
    }
    let cam = scene.add_entity("Cam".into());
    scene
        .world
        .set_camera(cam, Some(CameraComponent::default()));
    scene
        .world
        .set_world_position(cam, Vec3::new(0.0, 1.0, -4.0));
    scene
}

/// A project of `walls.scene` (the wall at `wall_x`) beside `pvs` as its `.occlusion`.
fn project(name: &str, wall_x: f32, pvs: &[u8]) -> std::path::PathBuf {
    let root = rusty_test_support::temp::dir().join(format!("baked_{name}"));
    let _ = std::fs::remove_dir_all(&root);
    let scenes = root.join("assets/scenes");
    std::fs::create_dir_all(&scenes).unwrap();
    let path = scenes.join("walls.scene").to_string_lossy().into_owned();
    rusty_edit::save_to_file(&walled(wall_x), &path).unwrap();
    std::fs::write(occlusion_path(&path), pvs).unwrap();
    root
}

fn baked(root: &Path) -> Vec<String> {
    let out = root.join("out");
    let report = bench(
        root,
        "walls",
        ProjectSettings::default(),
        &out,
        OPTS,
        &Extra::default(),
    );
    let report = report.unwrap();
    report
        .warnings
        .into_iter()
        .filter(|w| w.starts_with("baked "))
        .collect()
}

fn pvs() -> Vec<u8> {
    encode(
        &bake_occlusion(&walled(0.0), 1)
            .0
            .expect("the wall occludes"),
    )
}

#[test]
fn gpu_a_current_pvs_says_nothing_and_a_refused_one_says_why() {
    assert_eq!(
        baked(&project("current", 0.0, &pvs())),
        Vec::<String>::new()
    );
    let refused = baked(&project("refused", 0.0, b"not a pvs"));
    assert_eq!(refused.len(), 1, "{refused:?}");
    assert!(
        refused[0].starts_with("baked occlusion not used: refused ("),
        "{refused:?}"
    );
}

#[test]
fn gpu_a_pvs_whose_occluder_moved_since_says_stale() {
    let stale = baked(&project("stale", 3.0, &pvs()));
    // The renderer judges it: with no adapter, nothing is drawn and nothing judged.
    let Some(line) = stale.first() else {
        return assert!(!rusty_render::gpu_required(), "no line");
    };
    assert!(
        line.starts_with("baked occlusion not used: stale (1 of the 1 "),
        "{stale:?}"
    );
}

#[test]
fn gpu_a_navmesh_plays_start_refused_says_so() {
    let root = project("navmesh", 0.0, &pvs());
    let scene = root.join("assets/scenes/walls.scene");
    let navmesh = rusty_scene::nav_settings::navmesh_path(&scene.to_string_lossy());
    std::fs::write(navmesh, b"not a navmesh").unwrap();
    let lines = baked(&root);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].starts_with("baked navmesh not used: refused at Play's start"));
}

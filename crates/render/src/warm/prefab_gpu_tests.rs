//! A level built from linked `.prefab` instances (#2123): activation rebuilds each
//! instance from its source, and its primitives carry their one revision, so the
//! warm-up's meshes are what the first frame draws and it uploads nothing.

use super::warm_gpu_tests::{first_frame, prepare, render, warm};
use crate::stats::Uploads;
use rusty_scene::authoring::{create_entity, Primitive};
use rusty_scene::{load_and_instantiate_linked, save_prefab, Scene};

/// `<tmp>/rusty_2123_prefab/level.scene`: two linked instances of a Capsule
/// holding a Cylinder.
fn level() -> String {
    let dir = rusty_test_support::temp::dir().join("rusty_2123_prefab");
    std::fs::create_dir_all(&dir).unwrap();
    let prefab = rusty_core::paths::to_slash(&dir.join("prop.prefab"));
    let mut source = Scene::new();
    let root = create_entity(&mut source, "Prop", Some(Primitive::Capsule));
    let child = create_entity(&mut source, "Lamp", Some(Primitive::Cylinder));
    source.set_parent(child, Some(root)).unwrap();
    save_prefab(&source, root, &prefab).unwrap();
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    for x in [-1.0, 1.0] {
        let id = load_and_instantiate_linked(&mut scene, &prefab, None).unwrap();
        let place = &mut scene.world.transform_mut(id).unwrap().position;
        *place = glam::Vec3::new(x, 0.0, -4.0);
    }
    let path = rusty_core::paths::to_slash(&dir.join("level.scene"));
    scene.save_to_file(&path).unwrap();
    path
}

#[test]
fn gpu_a_level_of_linked_prefabs_uploads_nothing_on_its_first_frame() {
    let Some(mut renderer) = crate::test_gpu::headless_or_skip(32, 32) else {
        return;
    };
    let path = level();
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    render(&mut renderer, &scene);
    let prepared = prepare(&path);
    assert!(prepared
        .data()
        .entities
        .iter()
        .any(|e| e.prefab_link.is_some()));
    assert_eq!(
        warm(&mut renderer, &prepared).count,
        2,
        "a capsule, a cylinder"
    );
    let first = first_frame(&mut renderer, &mut scene, prepared);
    assert_eq!(
        first,
        Uploads::default(),
        "the propagated instances are resident"
    );
}

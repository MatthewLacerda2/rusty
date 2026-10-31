//! A material edited on a placed model is the scene's (#1637, Unity's extracted
//! material): propagation reads it and never swaps it for the model's unedited
//! copy, on Play→Stop, on load and on a Scale Factor change, and no `(n)` key
//! appears.

use glam::Vec3;

use super::tests::{child, spawn_file, KIT};
use crate::scene::asset_instance::set_model_scale_factor;
use crate::scene::serialize::{apply_scene_data, to_scene_data};
use crate::scene::snapshot::SceneSnapshot;
use crate::scene::Scene;

const RED: [f32; 3] = [1.0, 0.0, 0.0];

/// The kit with its `Wall` drawn with a glTF material named `Wood`.
fn wood_kit() -> String {
    KIT.replace(
        r#"{ "name": "WallMesh", "primitives": [ { "attributes": { "POSITION": 0 } } ] }"#,
        r#"{ "name": "WallMesh", "primitives": [ { "attributes": { "POSITION": 0 }, "material": 0 } ] }"#,
    )
    .replace(r#""meshes": ["#, r#""materials": [ { "name": "Wood" } ], "meshes": ["#)
}

/// The Wall's material key and colour, and every library key that copies it.
fn wall_material(scene: &Scene, root: u32) -> (String, [f32; 3], Vec<String>) {
    let wall = child(scene, child(scene, root, "Kit"), "Wall");
    let key = scene.world.material(wall).unwrap().materials[0].clone();
    let colour = scene.materials[&key].base_color;
    let prefix = format!("{key} (");
    let copies = scene.materials.keys().filter(|k| k.starts_with(&prefix));
    let copies = copies.cloned().collect();
    (key, colour, copies)
}

#[test]
fn a_recoloured_part_keeps_its_colour_through_stop_load_and_scale() {
    for (file, part) in [
        ("rusty_1637_wood.gltf", true),
        ("rusty_1637_bare.gltf", false),
    ] {
        let gltf = match part {
            true => wood_kit(),
            false => KIT.to_string(),
        };
        let (mut scene, root) = spawn_file(file, &gltf, Vec3::ZERO);
        let (key, _, _) = wall_material(&scene, root);
        assert!(
            key.ends_with(if part { "::Wood" } else { "::#default" }),
            "{key}"
        );
        scene.materials.get_mut(&key).unwrap().base_color = RED;

        let snapshot = SceneSnapshot::capture(&scene);
        snapshot.restore(&mut scene);
        assert_eq!(
            wall_material(&scene, root),
            (key.clone(), RED, vec![]),
            "Stop"
        );

        for _ in 0..2 {
            let data = to_scene_data(&scene);
            scene = Scene::new();
            apply_scene_data(&mut scene, data);
            assert_eq!(
                wall_material(&scene, root),
                (key.clone(), RED, vec![]),
                "load"
            );
        }

        let path = scene.world.prefab_link(root).unwrap().source.clone();
        assert!(set_model_scale_factor(&mut scene, &path, 0.5).unwrap() > 0);
        assert_eq!(
            wall_material(&scene, root),
            (key.clone(), RED, vec![]),
            "scale"
        );
    }
}

#[test]
fn a_second_spawn_shares_the_edited_material() {
    let (mut scene, root) = spawn_file("rusty_1637_wood2.gltf", &wood_kit(), Vec3::ZERO);
    let (key, _, _) = wall_material(&scene, root);
    scene.materials.get_mut(&key).unwrap().base_color = RED;
    let path = scene.world.prefab_link(root).unwrap().source.clone();
    let asset = crate::asset::imported(std::path::Path::new(&path)).unwrap();
    let again = super::instantiate_model(&mut scene, &path, &asset, Vec3::X);
    assert_eq!(wall_material(&scene, again), (key, RED, vec![]));
}

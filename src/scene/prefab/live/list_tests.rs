//! Lists and the three override shapes through Play and a save/load (#1527): a list
//! keeps a size entry and its new elements only, so its other elements keep
//! following the source; and modifications, added and removed components round-trip
//! together.

use super::component_tests::{bare, play_stop};
use super::tests::save_and_load;
use crate::components::{CameraComponent, ScriptComponent, ScriptFieldValue};
use crate::scene::authoring::defaults::default_collider;
use crate::scene::prefab::{read_prefab_file, write_prefab_file};
use crate::scene::{list_instance_overrides, Scene};
use glam::Vec3;

/// A script at `path`, its `speed` field set when given.
fn script(path: &str, speed: Option<f64>) -> ScriptComponent {
    let values = speed.map(|s| ("speed".to_string(), ScriptFieldValue::Number(s)));
    ScriptComponent {
        path: path.into(),
        values: values.into_iter().collect(),
        ..ScriptComponent::default()
    }
}

/// `child`'s scripts as (path, speed).
fn scripts(scene: &Scene, child: u32) -> Vec<(String, Option<ScriptFieldValue>)> {
    let list = scene.world.scripts(child).unwrap();
    let speed = |s: &ScriptComponent| s.values.get("speed").cloned();
    list.iter().map(|s| (s.path.clone(), speed(s))).collect()
}

#[test]
fn removing_the_first_of_two_scripts_survives_play_and_reload() {
    let (mut scene, root, child) = bare("first_script", |s, c| {
        let mut list = s.world.scripts_mut(c).unwrap();
        list.extend([script("a.lua", Some(1.0)), script("b.lua", None)]);
    });
    scene.world.scripts_mut(child).unwrap().remove(0);
    let listed = list_instance_overrides(&scene, root).unwrap();
    assert!(
        listed.contains(&"1:/scripts/#size".to_string()),
        "{listed:?}"
    );

    play_stop(&mut scene);
    let back = save_and_load(&scene);
    for scene in [&scene, &back] {
        assert_eq!(scripts(scene, child), [("b.lua".to_string(), None)]);
        assert_eq!(list_instance_overrides(scene, root).unwrap(), listed);
    }
}

#[test]
fn a_grown_list_keeps_following_the_source_on_its_untouched_element() {
    let (mut scene, root, child) = bare("grown_follow", |s, c| {
        s.world.scripts_mut(c).unwrap().push(script("a.lua", None));
    });
    scene
        .world
        .scripts_mut(child)
        .unwrap()
        .push(script("b.lua", None));
    let grown = ["1:/scripts/#size", "1:/scripts/1"];
    assert_eq!(list_instance_overrides(&scene, root).unwrap(), grown);

    // The source edits the element both lists hold; Stop re-propagates it.
    let path = crate::test_temp::dir().join("rusty_1527_grown_follow.prefab");
    let path = path.to_string_lossy().into_owned();
    let mut source = read_prefab_file(&path).unwrap();
    source.entities[1].scripts[0] = script("a.lua", Some(5.0));
    write_prefab_file(&source, &path).unwrap();
    play_stop(&mut scene);

    let speed = Some(ScriptFieldValue::Number(5.0));
    let want = [("a.lua".to_string(), speed), ("b.lua".to_string(), None)];
    assert_eq!(scripts(&scene, child), want, "the source's edit reached it");
    assert_eq!(list_instance_overrides(&scene, root).unwrap(), grown);
}

#[test]
fn all_three_kinds_round_trip_through_play_and_reload() {
    let (mut scene, root, child) = bare("three_kinds", |s, c| {
        s.world.set_collider(c, Some(default_collider()));
        s.world.scripts_mut(c).unwrap().push(script("a.lua", None));
    });
    scene.world.transform_mut(child).unwrap().scale = Vec3::splat(2.0);
    scene.world.set_collider(child, None);
    let camera = CameraComponent {
        fov: 75.0,
        ..CameraComponent::default()
    };
    scene.world.set_camera(child, Some(camera));
    scene.world.scripts_mut(child).unwrap().clear();
    let listed = list_instance_overrides(&scene, root).unwrap();
    for key in [
        "1:/transform/scale/0",
        "1:/scripts/#size",
        "1:+/camera",
        "1:-/collider",
    ] {
        assert!(listed.contains(&key.to_string()), "{key} in {listed:?}");
    }

    play_stop(&mut scene);
    let back = save_and_load(&scene);
    for scene in [&scene, &back] {
        let scale = scene.world.transform(child).unwrap().scale;
        assert_eq!(scale, Vec3::splat(2.0));
        assert!(
            scene.world.collider(child).is_none(),
            "the collider stays removed"
        );
        assert_eq!(
            scene.world.camera(child).expect("the camera stays").fov,
            75.0
        );
        assert!(scene.world.scripts(child).unwrap().is_empty());
        assert_eq!(list_instance_overrides(scene, root).unwrap(), listed);
    }
}

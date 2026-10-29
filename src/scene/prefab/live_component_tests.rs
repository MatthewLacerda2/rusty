//! A component added to an instance is one override holding its whole value (#1527),
//! as Unity's *added component*: a Light, a Collider or a script added to an instance
//! child keeps its values through Play and a save/load.

use super::live_tests::save_and_load;
use crate::components::{ColliderShape, ScriptComponent, ScriptFieldValue};
use crate::scene::authoring::defaults::{default_collider, default_light};
use crate::scene::snapshot::SceneSnapshot;
use crate::scene::{list_instance_overrides, load_and_instantiate_linked, save_prefab, Scene};
use glam::Vec3;

/// A linked instance of `Root { Child }`, the child shaped by `dress` before the
/// prefab is saved: (scene, root, child).
pub(super) fn bare(tag: &str, dress: impl FnOnce(&mut Scene, u32)) -> (Scene, u32, u32) {
    let mut author = Scene::new();
    let root = author.add_entity("Root".into());
    let child = author.add_entity("Child".into());
    author.set_parent(child, Some(root)).unwrap();
    dress(&mut author, child);
    let path = crate::test_temp::dir().join(format!("rusty_1527_{tag}.prefab"));
    let path = path.to_string_lossy().into_owned();
    save_prefab(&author, root, &path).unwrap();
    let mut scene = Scene::new();
    let root = load_and_instantiate_linked(&mut scene, &path, None).unwrap();
    let child = scene.world.children(root)[0];
    (scene, root, child)
}

/// Enter Play and leave it: the edit-mode World captured and put back.
pub(super) fn play_stop(scene: &mut Scene) {
    SceneSnapshot::capture(scene).restore(scene);
}

#[test]
fn an_added_light_collider_and_script_survive_play_and_reload() {
    let (mut scene, root, child) = bare("added", |_, _| {});
    let mut light = default_light();
    light.color = Vec3::new(1.0, 0.2, 0.1);
    scene.world.set_light(child, Some(light));
    let mut collider = default_collider();
    collider.shape = ColliderShape::Sphere { radius: 2.5 };
    collider.is_trigger = true;
    scene.world.set_collider(child, Some(collider));
    let script = ScriptComponent {
        path: "scripts/spin.lua".into(),
        values: [("speed".into(), ScriptFieldValue::Number(3.0))].into(),
        ..ScriptComponent::default()
    };
    scene.world.scripts_mut(child).unwrap().push(script);
    let whole = ["1:/collider", "1:/light", "1:/scripts"];
    assert_eq!(list_instance_overrides(&scene, root).unwrap(), whole);

    play_stop(&mut scene);
    let back = save_and_load(&scene);
    for scene in [&scene, &back] {
        let light = scene.world.light(child).expect("the light survives");
        assert_eq!(light.color, Vec3::new(1.0, 0.2, 0.1));
        let collider = scene.world.collider(child).expect("the collider survives");
        assert_eq!(collider.shape, ColliderShape::Sphere { radius: 2.5 });
        assert!(collider.is_trigger);
        let scripts = scene.world.scripts(child).unwrap();
        assert_eq!(scripts.len(), 1, "the script survives");
        assert_eq!(scripts[0].path, "scripts/spin.lua");
        let speed = &scripts[0].values["speed"];
        assert_eq!(*speed, ScriptFieldValue::Number(3.0));
        assert_eq!(list_instance_overrides(scene, root).unwrap(), whole);
    }
}

#[test]
fn a_second_script_on_an_instance_survives_play() {
    let first = ScriptComponent {
        path: "scripts/a.lua".into(),
        ..ScriptComponent::default()
    };
    let (mut scene, root, child) = bare("grown", |s, c| {
        s.world.scripts_mut(c).unwrap().push(first.clone());
    });
    let second = ScriptComponent {
        path: "scripts/b.lua".into(),
        ..ScriptComponent::default()
    };
    scene.world.scripts_mut(child).unwrap().push(second);
    assert_eq!(
        list_instance_overrides(&scene, root).unwrap(),
        ["1:/scripts"]
    );
    play_stop(&mut scene);
    let paths: Vec<_> = scene
        .world
        .scripts(child)
        .unwrap()
        .iter()
        .map(|s| s.path.clone())
        .collect();
    assert_eq!(paths, ["scripts/a.lua", "scripts/b.lua"]);
}

#[test]
fn a_removed_component_is_one_null_override() {
    let (mut scene, root, child) = bare("removed", |s, c| {
        s.world.set_light(c, Some(default_light()));
    });
    scene.world.set_light(child, None);
    assert_eq!(list_instance_overrides(&scene, root).unwrap(), ["1:/light"]);
    play_stop(&mut scene);
    assert!(scene.world.light(child).is_none(), "it stays removed");
    let back = save_and_load(&scene);
    assert!(back.world.light(child).is_none());
}

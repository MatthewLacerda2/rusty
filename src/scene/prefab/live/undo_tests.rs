//! The live diff is the override set (#1527): an edit the author undoes leaves no
//! override behind, through Play and through a save/load, so it never comes back.

use super::component_tests::{bare, play_stop};
use super::tests::save_and_load;
use crate::components::ColliderShape;
use crate::scene::authoring::defaults::{default_collider, default_light};
use crate::scene::{list_instance_overrides, Scene};
use glam::Vec3;

/// No override on the instance rooted at `root`, now and after a save/load.
fn clean(scene: &Scene, root: u32) -> Scene {
    assert!(list_instance_overrides(scene, root).unwrap().is_empty());
    let back = save_and_load(scene);
    assert!(list_instance_overrides(&back, root).unwrap().is_empty());
    back
}

#[test]
fn a_component_removed_then_re_added_leaves_no_override() {
    let (mut scene, root, child) = bare("readded", |s, c| {
        s.world.set_light(c, Some(default_light()));
    });
    scene.world.set_light(child, None);
    play_stop(&mut scene);
    scene.world.set_light(child, Some(default_light()));
    play_stop(&mut scene);
    assert!(scene.world.light(child).is_some(), "the light is back");
    let back = clean(&scene, root);
    assert!(back.world.light(child).is_some());
}

#[test]
fn a_collider_shape_switched_and_back_leaves_no_override() {
    let (mut scene, root, child) = bare("shape", |s, c| {
        s.world.set_collider(c, Some(default_collider()));
    });
    let sphere = ColliderShape::Sphere { radius: 2.0 };
    scene.world.collider_mut(child).unwrap().shape = sphere.clone();
    play_stop(&mut scene);
    assert_eq!(
        scene.world.collider(child).unwrap().shape,
        sphere,
        "the switch survives"
    );
    let boxed = ColliderShape::Box { size: Vec3::ONE };
    scene.world.collider_mut(child).unwrap().shape = boxed.clone();
    play_stop(&mut scene);
    let back = clean(&scene, root);
    assert_eq!(back.world.collider(child).unwrap().shape, boxed);
}

#[test]
fn a_component_added_then_removed_leaves_no_override() {
    let (mut scene, root, child) = bare("unadded", |_, _| {});
    scene.world.set_light(child, Some(default_light()));
    play_stop(&mut scene);
    scene.world.set_light(child, None);
    play_stop(&mut scene);
    assert!(scene.world.light(child).is_none(), "the light stays gone");
    let back = clean(&scene, root);
    assert!(back.world.light(child).is_none());
}

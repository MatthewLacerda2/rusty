use glam::Vec3;

use super::super::super::test_support::{add_box, add_box_with, add_floor};
use super::super::super::{NavBounds, NavigationGraph};
use rusty_ecs::walks;
use rusty_scene::Scene;

fn yard() -> (Scene, NavigationGraph, u32, u32, Vec<u32>) {
    let mut scene = Scene::new();
    scene.nav_settings.grid_spacing = 0.5;
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 6.0, 0.0, 6.0));
    add_floor(&mut scene, 0.0, 6.0, 0.0, 6.0);
    let mover = scene.add_entity("mover".to_string());
    let crate_ = add_box(
        &mut scene,
        Vec3::new(1.0, 0.0, 1.0),
        Vec3::new(2.0, 1.0, 2.0),
    );
    scene.set_parent(crate_, Some(mover)).unwrap();
    let min = Vec3::new(4.0, 0.0, 4.0);
    let mut rig = vec![add_box_with(&mut scene, min, min + Vec3::ONE, false)];
    for _ in 0..3 {
        let bone = add_box_with(&mut scene, min, min + Vec3::splat(0.2), false);
        scene.set_parent(bone, rig.last().copied()).unwrap();
        rig.push(bone);
    }
    let mut g = NavigationGraph::from_scene(&scene);
    g.start_tracked(&mut scene);
    assert!(still(&mut scene, &mut g), "the watch has read the setup");
    (scene, g, mover, crate_, rig)
}

fn still(s: &mut Scene, g: &mut NavigationGraph) -> bool {
    g.static_watch.as_mut().unwrap().still(&mut s.world)
}

#[test]
fn a_static_collider_moved_by_its_non_static_parent_is_a_change() {
    let (mut scene, mut g, mover, ..) = yard();
    scene.world.transform_mut(mover).unwrap().position.x += 0.5;
    assert!(
        !still(&mut scene, &mut g),
        "the crate moved with its parent"
    );
    assert!(still(&mut scene, &mut g), "and was read");
}

#[test]
fn a_static_moved_again_before_another_reader_reads_is_a_change() {
    let (mut scene, mut g, mover, ..) = yard();
    let mut other = scene.world.change_cursor::<rusty_ecs::WorldPose>();
    scene.world.transform_mut(mover).unwrap().position.x += 0.5;
    assert!(!still(&mut scene, &mut g));
    scene.world.transform_mut(mover).unwrap().position.x += 0.5;
    assert!(
        !still(&mut scene, &mut g),
        "moved again, `other` still unread"
    );
    let _ = scene.world.changes(&mut other);
}

#[test]
fn a_rig_animating_beside_the_floor_is_still_and_collects_no_ids() {
    let (mut scene, mut g, _, _, rig) = yard();
    let before = walks::ids_collected();
    for tick in 0..4 {
        for &bone in &rig {
            scene.world.transform_mut(bone).unwrap().position.y += 0.01 * (tick + 1) as f32;
        }
        assert!(still(&mut scene, &mut g), "tick {tick}: only the rig moved");
    }
    assert_eq!(walks::ids_collected(), before, "the watch listed no id");
}

#[test]
fn a_collider_edited_on_a_static_entity_is_a_change() {
    let (mut scene, mut g, _, crate_, rig) = yard();
    scene.world.collider_mut(rig[0]).unwrap().is_trigger = true;
    assert!(still(&mut scene, &mut g), "a dynamic collider edited");
    scene.world.collider_mut(crate_).unwrap().is_trigger = true;
    assert!(!still(&mut scene, &mut g), "the crate turned trigger");
    scene.world.set_collider(crate_, None);
    assert!(!still(&mut scene, &mut g), "the crate lost its collider");
}

#[test]
fn an_activity_toggle_above_a_static_collider_is_a_change() {
    let (mut scene, mut g, mover, _, rig) = yard();
    scene.world.set_active(rig[1], false);
    assert!(still(&mut scene, &mut g), "no static below the bone");
    for active in [false, true] {
        scene.world.set_active(mover, active);
        assert!(!still(&mut scene, &mut g), "the crate's parent -> {active}");
    }
}

#[test]
fn a_rewound_or_cleared_world_is_a_change() {
    let (mut scene, mut g, ..) = yard();
    scene.world.rewind_changes();
    assert!(!still(&mut scene, &mut g), "an Undo step");
    let entities = scene.world.collect_entities();
    scene.world.clear();
    entities
        .into_iter()
        .for_each(|e| scene.world.insert_entity(e));
    assert!(!still(&mut scene, &mut g), "cleared and filled again");
    assert!(still(&mut scene, &mut g), "and was read");
}

#[test]
fn a_replaced_world_is_a_change() {
    let (mut scene, mut g, ..) = yard();
    scene.world = yard().0.world;
    assert!(!still(&mut scene, &mut g), "another World");
}

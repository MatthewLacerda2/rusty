//! What a play tick's spawns and modifier writes cost the navmesh's static watch
//! (`bake::still`): entities spawned and destroyed with no `NavMeshModifier` and no
//! static flag (casings, decals) leave it still and skip the static collider diff;
//! a modifier's rewrite of its own value is still; a modifier added, edited or
//! removed on a non-static parent, and a static collider spawned or destroyed, are
//! changes.

use glam::Vec3;

use super::super::super::test_support::{add_box, add_box_with, add_floor};
use super::super::super::{NavBounds, NavigationGraph};
use super::super::sync::STATIC_DIFFS;
use rusty_components::NavMeshModifierComponent;
use rusty_scene::Scene;

/// A floor and a static crate under a non-static `holder`: `(scene, graph, holder)`,
/// the watch opened and read.
fn yard() -> (Scene, NavigationGraph, u32) {
    let mut scene = Scene::new();
    scene.nav_settings.grid_spacing = 0.5;
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 6.0, 0.0, 6.0));
    add_floor(&mut scene, 0.0, 6.0, 0.0, 6.0);
    let holder = scene.add_entity("holder".to_string());
    let crate_ = add_box(
        &mut scene,
        Vec3::new(1.0, 0.0, 1.0),
        Vec3::new(2.0, 1.0, 2.0),
    );
    scene.set_parent(crate_, Some(holder)).unwrap();
    let mut g = NavigationGraph::from_scene(&scene);
    g.start_tracked(&mut scene);
    assert!(still(&mut scene, &mut g), "the watch has read the setup");
    (scene, g, holder)
}

fn still(s: &mut Scene, g: &mut NavigationGraph) -> bool {
    g.static_watch.as_mut().unwrap().still(&mut s.world)
}

fn diffs() -> u64 {
    STATIC_DIFFS.with(|n| n.get())
}

#[test]
fn spawning_and_destroying_dynamic_entities_skips_the_static_diff() {
    let (mut scene, mut g, _) = yard();
    let before = diffs();
    let mut casings = Vec::new();
    for tick in 0..4 {
        let at = Vec3::new(3.0 + 0.1 * tick as f32, 0.0, 3.0);
        casings.push(add_box_with(&mut scene, at, at + Vec3::splat(0.05), false));
        casings.push(scene.add_entity("decal".to_string()));
        if tick > 1 {
            scene.world.despawn(casings.remove(0));
        }
        g.sync_changes(&mut scene);
    }
    assert_eq!(diffs(), before, "no static collider diff ran");
    assert!(still(&mut scene, &mut g), "and nothing is left unread");
}

#[test]
fn a_modifier_rewritten_with_its_own_value_is_still() {
    let (mut scene, mut g, holder) = yard();
    let modifier = NavMeshModifierComponent::default();
    scene.world.set_nav_mesh_modifier(holder, Some(modifier));
    assert!(!still(&mut scene, &mut g), "a modifier added on the parent");
    let _ = scene.world.nav_mesh_modifier_mut(holder); // a borrow, no write
    assert!(still(&mut scene, &mut g), "the same value");
    scene
        .world
        .nav_mesh_modifier_mut(holder)
        .unwrap()
        .ignore_from_build = true;
    assert!(!still(&mut scene, &mut g), "an edit");
    scene.world.set_nav_mesh_modifier(holder, None);
    assert!(!still(&mut scene, &mut g), "a removal");
    assert!(still(&mut scene, &mut g), "and was read");
}

#[test]
fn a_static_collider_spawned_or_destroyed_in_play_is_a_change() {
    let (mut scene, mut g, _) = yard();
    let at = Vec3::new(4.0, 0.0, 4.0);
    let wall = add_box(&mut scene, at, at + Vec3::ONE);
    assert!(!still(&mut scene, &mut g), "a static wall spawned");
    scene.world.despawn(wall);
    assert!(!still(&mut scene, &mut g), "and destroyed");
    assert!(still(&mut scene, &mut g), "and was read");
}

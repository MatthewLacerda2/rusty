//! Off-mesh link tests (#462). Levels are primitive boxes only: a floor, a 3 m
//! platform with a ramp up its far side, a deck reached by nothing but a ladder.

mod generate;
mod incremental;
mod routes;
mod traverse;

use glam::{Quat, Vec3};

use super::super::test_support::{add_box, add_floor, add_rotated_box};
use super::super::{NavBounds, NavigationGraph};
use super::{OffMeshLink, OffMeshLinkKind};
use crate::components::OffMeshLinkComponent;
use crate::scene::Scene;

/// A 20 × 20 floor, a 3 m platform over x 0..8, z 0..8, and a ramp from the
/// platform's +z edge down to the floor at z = 16. Drops and jump-ups enabled.
fn platform() -> Scene {
    let mut scene = Scene::new();
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 20.0, 0.0, 20.0));
    scene.nav_settings.drop_height = 4.0;
    scene.nav_settings.jump_height = 1.2;
    scene.nav_settings.jump_distance = 2.0;
    add_floor(&mut scene, -1.0, 21.0, -1.0, 21.0);
    add_box(&mut scene, Vec3::ZERO, Vec3::new(8.0, 3.0, 8.0));
    let tilt = (3.0f32 / 8.0).atan();
    add_rotated_box(
        &mut scene,
        Vec3::new(3.5, 1.4, 12.0),
        Vec3::new(3.0, 0.2, 8.6),
        Quat::from_rotation_x(tilt),
        true,
    );
    scene
}

/// A floor and a deck at y = 4 over x 10..20 with no way up; returns the scene
/// and a ladder link from the ground at x = 8 to the deck at x = 12.
fn ladder() -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.nav_settings.bounds = Some(NavBounds::new(0.0, 20.0, 0.0, 10.0));
    add_floor(&mut scene, -1.0, 21.0, -1.0, 11.0);
    add_box(
        &mut scene,
        Vec3::new(10.0, 3.8, -1.0),
        Vec3::new(21.0, 4.0, 11.0),
    );
    let id = scene.add_entity("ladder".to_string());
    if let Some(mut t) = scene.world.transform_mut(id) {
        t.position = Vec3::new(8.0, 0.0, 5.0);
    }
    let link = OffMeshLinkComponent {
        end: Vec3::new(4.0, 4.0, 0.0),
        ..Default::default()
    };
    scene.world.set_offmesh_link(id, Some(link));
    (scene, id)
}

fn baked(scene: &Scene) -> NavigationGraph {
    let mut g = NavigationGraph::from_scene(scene);
    g.bake(scene);
    g
}

fn links(g: &NavigationGraph) -> Vec<OffMeshLink> {
    g.offmesh_links().cloned().collect()
}

fn of_kind(g: &NavigationGraph, kind: OffMeshLinkKind) -> Vec<OffMeshLink> {
    links(g).into_iter().filter(|l| l.kind == kind).collect()
}

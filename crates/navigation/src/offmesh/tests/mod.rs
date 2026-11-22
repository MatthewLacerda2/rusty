mod generate;
mod incremental;
mod routes;
mod traverse;

use glam::{Quat, Vec3};

use super::super::test_support::{add_box, add_floor, add_rotated_box};
use super::super::{NavBounds, NavigationGraph};
use super::{OffMeshLink, OffMeshLinkKind};
use rusty_components::OffMeshLinkComponent;
use rusty_scene::Scene;

fn platform() -> Scene {
    let mut scene = Scene::new();
    scene.nav_settings.grid_spacing = 1.0;
    scene.nav_settings.max_step = 0.5; // the ramp climbs 0.375 a 1 m cell
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

fn ladder() -> (Scene, u32) {
    let mut scene = Scene::new();
    scene.nav_settings.grid_spacing = 1.0;
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

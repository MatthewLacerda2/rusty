use glam::{Vec2, Vec3};

use super::tests::{bake_reaching, floor, hangar, LAMP};
use super::{play_height, Reach, MIN_PLAY_HEIGHT};
use crate::occlusion::{bake_occlusion, is_current};
use rusty_scene::lighting::floor::{FloorCell, FloorGrid};
use rusty_scene::nav_settings::NavMeshSettings;
use rusty_scene::Scene;

fn room(inset: f32) -> FloorGrid {
    let open = FloorCell {
        floor: 0.0,
        ceiling: f32::INFINITY,
    };
    FloorGrid {
        origin: Vec2::splat(0.25),
        cell: 0.5,
        width: 11,
        depth: 16,
        cells: vec![Some(open); 11 * 16],
        upper: Vec::new(),
        inset,
    }
}

fn standable_at(scene: &Scene, floor: &FloorGrid, at: Vec3) -> bool {
    let reach = Reach::over(scene, floor);
    let cells = reach
        .grid(Vec3::ZERO, Vec3::new(8.0, 4.0, 8.0))
        .expect("a grid");
    let standable = reach.standable(&cells.grid).expect("a floor");
    standable[cells.grid.index_of(at).expect("inside")]
}

#[test]
fn the_strip_beside_a_wall_is_standable() {
    let scene = Scene::new();
    let strip = Vec3::new(7.0, 1.0, 4.0);
    assert!(standable_at(&scene, &room(0.75), strip));
    assert!(
        !standable_at(&scene, &room(0.0), strip),
        "no inset, no strip"
    );
}

#[test]
fn the_play_height_follows_the_navmesh_agent_and_its_jump() {
    let mut s = NavMeshSettings::default();
    assert_eq!(
        play_height(&s),
        3.0,
        "the defaults: a 2 m agent and a 1 m jump"
    );
    (s.agent_height, s.jump_height) = (2.5, 1.5);
    assert_eq!(play_height(&s), 4.0);
    (s.agent_height, s.jump_height) = (0.5, 0.0);
    assert_eq!(
        play_height(&s),
        MIN_PLAY_HEIGHT,
        "never under the floor of it"
    );
    s.agent_height = f32::NAN;
    assert_eq!(play_height(&s), MIN_PLAY_HEIGHT);

    let mut scene = Scene::new();
    let high = Vec3::new(3.0, 5.0, 3.0); // the layer from y 4 to 6
    assert!(
        !standable_at(&scene, &room(0.0), high),
        "out of a 3 m reach"
    );
    scene.nav_settings.agent_height = 3.5;
    assert!(standable_at(&scene, &room(0.0), high), "in a 4.5 m one");
}

#[test]
fn the_play_height_joins_the_key() {
    let mut scene = Scene::new();
    let (min, max) = (Vec3::ZERO, Vec3::new(8.0, 4.0, 8.0));
    let key = |scene: &Scene| {
        Reach::over(scene, &room(0.0))
            .grid(min, max)
            .expect("a grid")
    };
    let before = key(&scene);
    scene.nav_settings.agent_height = 2.1; // the same cells marked
    let after = key(&scene);
    assert_eq!(before.sampled, after.sampled);
    assert_ne!(before.key, after.key, "a changed agent height re-bakes");
}

const PERCH: Vec3 = Vec3::new(5.0, 8.5, 5.0);

fn watched(at: Vec3) -> Scene {
    let mut scene = hangar(LAMP);
    let id = scene.add_entity("Overhead".into());
    let camera = rusty_components::CameraComponent::default();
    scene.world.set_camera(id, Some(camera));
    scene.world.transform_mut(id).expect("new").position = at;
    scene.world.set_active(id, false);
    scene
}

#[test]
fn a_cameras_cell_is_sampled_and_keeps_its_full_bits() {
    let scene = watched(PERCH);
    let full = bake_occlusion(&scene, 2).0.expect("occluders");
    let (data, sampled) = bake_reaching(&scene, Reach::over(&scene, &floor(&scene)));
    let cell = data.grid.index_of(PERCH).expect("in the grid");
    assert!(
        sampled.contains(&cell),
        "an inactive camera's cell is sampled"
    );
    let row = |d: &rusty_scene::lighting::occlusion::OcclusionData| {
        let row = d.row_at(PERCH).expect("a row");
        (0..d.objects.len() as u32)
            .map(|i| d.sees(row, i))
            .collect::<Vec<_>>()
    };
    assert_eq!(row(&data), row(&full));
}

#[test]
fn a_camera_moved_within_its_cell_keeps_and_to_another_cell_re_bakes() {
    let scene = watched(PERCH);
    let (held, _) = bake_reaching(&scene, Reach::over(&scene, &floor(&scene)));
    let near = PERCH + Vec3::splat(0.1);
    assert_eq!(held.grid.index_of(near), held.grid.index_of(PERCH));
    let current = |s: &Scene| is_current(s, &held, &Reach::over(s, &floor(s)));
    assert!(current(&watched(near)), "within its cell");
    assert!(
        !current(&watched(Vec3::new(-5.0, 8.5, 5.0))),
        "to another cell"
    );
}

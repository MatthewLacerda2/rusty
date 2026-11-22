use std::sync::{Arc, Mutex};

use glam::Vec3;

use crate::occlusion::test_level::block;
use crate::occlusion::{
    bake_occlusion, gather_with, Fingerprints, OcclusionObserver, OcclusionProgress, Reach,
};
use rusty_scene::create::{create_entity, Primitive};
use rusty_scene::lighting::floor::FloorGrid;
use rusty_scene::lighting::occlusion::{encode, FloorSource, OcclusionData};
use rusty_scene::Scene;

pub(super) const LAMP: Vec3 = Vec3::new(-7.0, 8.5, -7.0);
const AIR: Vec3 = Vec3::new(5.0, 6.8, 5.0);

pub(super) fn hangar(lamp: Vec3) -> Scene {
    let mut scene = Scene::new();
    scene.skybox_path = String::new();
    block(
        &mut scene,
        "Floor",
        Vec3::new(-8.0, -0.2, -8.0),
        Vec3::new(8.0, 0.0, 8.0),
    );
    block(
        &mut scene,
        "Wall",
        Vec3::new(-0.1, 0.0, -8.0),
        Vec3::new(0.1, 3.0, 8.0),
    );
    block(
        &mut scene,
        "Roof",
        Vec3::new(-8.0, 10.0, -8.0),
        Vec3::new(8.0, 10.2, 8.0),
    );
    for at in [Vec3::new(-5.0, 0.5, -5.0), Vec3::new(5.0, 0.5, 5.0)] {
        block(
            &mut scene,
            "Prop",
            at - Vec3::splat(0.5),
            at + Vec3::splat(0.5),
        );
    }
    let id = create_entity(&mut scene, "Lamp", Some(Primitive::PointLight));
    scene.world.transform_mut(id).expect("new").position = lamp;
    let mut light = scene.world.light(id).expect("a light").clone();
    light.cast_shadows = true;
    scene.world.set_light(id, Some(light));
    scene
}

pub(super) fn floor(scene: &Scene) -> FloorGrid {
    FloorGrid::from_static(scene, 0.5).expect("a floor")
}

#[derive(Default)]
struct Started(Mutex<Vec<usize>>);

impl OcclusionObserver for Started {
    fn cells(&self, _: usize) {}
    fn cell_started(&self, cell: usize) {
        self.0.lock().expect("one test").push(cell);
    }
    fn cell_finished(&self, _: usize) {}
}

pub(super) fn bake_reaching(scene: &Scene, reach: Reach) -> (OcclusionData, Vec<usize>) {
    let started = Arc::new(Started::default());
    let progress = OcclusionProgress::observed(started.clone());
    let input = gather_with(scene, &Fingerprints::of_scene(scene), reach);
    let (data, _) = input.bake(2, &progress).expect("not cancelled");
    let cells = started.0.lock().expect("one test").clone();
    (data.expect("occluders"), cells)
}

fn bits(data: &OcclusionData, eye: Vec3) -> Vec<bool> {
    let row = data.row_at(eye).expect("in the grid");
    (0..data.objects.len() as u32)
        .map(|i| data.sees(row, i))
        .collect()
}

#[test]
fn the_floor_keeps_a_full_bakes_bits_and_the_air_above_sees_everything() {
    let scene = hangar(LAMP);
    let full = bake_occlusion(&scene, 2).0.expect("occluders");
    let (data, sampled) = bake_reaching(&scene, Reach::over(&scene, &floor(&scene)));
    assert_eq!(data.grid, full.grid, "a small level keeps 2 m cells");
    assert!(sampled.len() < data.grid.cell_count(), "{}", sampled.len());
    for eye in [
        Vec3::new(-4.0, 1.6, 3.0),
        Vec3::new(4.0, 2.5, -6.0),
        Vec3::new(6.0, 0.1, 6.0),
    ] {
        assert_eq!(bits(&data, eye), bits(&full, eye), "a camera at {eye}");
    }
    let air = data.grid.index_of(AIR).expect("in the grid");
    assert!(!sampled.contains(&air), "the air is not sampled");
    assert!(bits(&data, AIR).iter().all(|&b| b), "and sees every object");
    let lamp = data.grid.index_of(LAMP).expect("in the grid");
    assert!(sampled.contains(&lamp), "the lamp's cell is sampled");
    assert_eq!(bits(&data, LAMP), bits(&full, LAMP), "its shadows' row");
    assert_eq!(data.stamp.baked as usize, sampled.len());
    assert_eq!(data.stamp.floor, FloorSource::Navmesh);
}

#[test]
fn no_floor_or_one_that_marks_no_cell_samples_every_cell() {
    let scene = hangar(LAMP);
    let full = encode(&bake_occlusion(&scene, 2).0.expect("occluders"));
    let mut far = floor(&scene);
    far.origin += glam::Vec2::splat(500.0);
    for reach in [Reach::everywhere(), Reach::over(&scene, &far)] {
        let (data, sampled) = bake_reaching(&scene, reach);
        assert_eq!(sampled.len(), data.grid.cell_count());
        assert_eq!(data.stamp.floor, FloorSource::Everywhere);
        assert_eq!(encode(&data), full, "the same bytes as before");
    }
}

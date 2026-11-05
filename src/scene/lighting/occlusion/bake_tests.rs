//! The occlusion bake (#1714): it culls behind walls, keeps what a doorway shows,
//! and writes the same bytes at any worker count.

use glam::Vec3;

use super::test_level::{block, corridors};
use super::{bake_occlusion, encode, fingerprint, OcclusionData};
use crate::scene::Scene;

fn baked(scene: &Scene, threads: usize) -> OcclusionData {
    bake_occlusion(scene, threads)
        .0
        .expect("the level has occluders")
}

/// Whether the entity named `name` nearest `near` may be seen from `from`.
fn sees(data: &OcclusionData, scene: &Scene, from: Vec3, near: Vec3) -> bool {
    let row = data.row_at(from).expect("inside the grid");
    let ids = scene.world.ids_with_mesh().into_iter();
    let nearest = ids.min_by(|&a, &b| {
        let d = |id| {
            scene
                .world
                .local_to_world(id)
                .w_axis
                .truncate()
                .distance(near)
        };
        d(a).total_cmp(&d(b))
    });
    let object = data.object_of(nearest.expect("a mesh")).expect("baked");
    data.sees(row, object)
}

#[test]
fn a_room_behind_a_wall_is_culled_and_one_through_a_door_is_not() {
    let scene = corridors();
    let data = baked(&scene, 4);
    // In the first north room's corner: the far south room's props are behind two
    // walls, the room's own props are in sight.
    let corner = Vec3::new(1.0, 1.6, 8.0);
    assert!(sees(&data, &scene, corner, Vec3::new(2.0, 0.5, 3.5)));
    assert!(!sees(&data, &scene, corner, Vec3::new(17.0, 0.5, -7.0)));
    // Down the corridor, both crates are in sight.
    let hall = Vec3::new(1.0, 1.6, 0.0);
    assert!(sees(&data, &scene, hall, Vec3::new(14.0, 0.25, 0.4)));
    let culled =
        (0..data.objects.len() as u32).filter(|&o| !data.sees(data.row_at(hall).unwrap(), o));
    assert!(
        culled.count() >= 6,
        "the rooms' far props hide from the corridor"
    );
}

#[test]
fn the_bake_is_the_same_bytes_at_any_worker_count() {
    let scene = corridors();
    let one = encode(&baked(&scene, 1));
    assert_eq!(one, encode(&baked(&scene, 3)));
    assert_eq!(one, encode(&baked(&scene, 16)));
}

#[test]
fn a_level_with_no_occluder_bakes_nothing() {
    let mut scene = Scene::new();
    assert_eq!(bake_occlusion(&scene, 2).0, None, "an empty level");
    let id = block(&mut scene, "Glass", Vec3::ZERO, Vec3::ONE);
    scene.world.set_static(id, false);
    let (data, report) = bake_occlusion(&scene, 2);
    assert_eq!(
        (data, report.objects),
        (None, 0),
        "a dynamic mesh takes no part"
    );
}

#[test]
fn a_move_changes_the_fingerprint_and_a_rename_does_not() {
    let mut scene = corridors();
    let id = scene.world.ids_with_mesh()[0];
    let before = fingerprint(&scene, id).expect("a static mesh");
    scene.world.set_name(id, "Renamed".into());
    assert_eq!(fingerprint(&scene, id), Some(before));
    scene.world.transform_mut(id).unwrap().position.x += 0.01;
    assert_ne!(fingerprint(&scene, id), Some(before));
    scene.world.set_static(id, false);
    assert_eq!(
        fingerprint(&scene, id),
        None,
        "a dynamic mesh is never baked"
    );
}

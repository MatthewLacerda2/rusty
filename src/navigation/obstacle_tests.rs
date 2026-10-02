//! `NavMeshObstacle` volumes, carving bookkeeping and the avoidance hook (#456).

use glam::{Mat4, Quat, Vec3};

use super::*;
use crate::components::NavMeshObstacleComponent;
use crate::navigation::test_support::{add_obstacle, move_to};

fn volume(shape: ObstacleShape, pose: Mat4) -> ObstacleVolume {
    let o = NavMeshObstacleComponent {
        shape,
        size: Vec3::new(2.0, 1.0, 4.0),
        ..Default::default()
    };
    ObstacleVolume::of(&o, pose)
}

#[test]
fn a_box_volume_follows_its_pose() {
    let pose =
        Mat4::from_rotation_translation(Quat::from_rotation_y(0.5), Vec3::new(3.0, 0.5, 1.0));
    let tris = volume(ObstacleShape::Box, pose).triangles();
    assert_eq!(tris.len(), 12);
    let ys: Vec<f32> = tris.iter().flatten().map(|v| v.y).collect();
    let (lo, hi) = ys
        .iter()
        .fold((f32::MAX, f32::MIN), |(a, b), &y| (a.min(y), b.max(y)));
    assert!((lo - 0.0).abs() < 1e-5 && (hi - 1.0).abs() < 1e-5);
}

#[test]
fn a_capsule_stays_upright_and_covers_its_disc() {
    let pose = Mat4::from_scale_rotation_translation(
        Vec3::new(2.0, 1.0, 1.0),
        Quat::from_rotation_z(1.0),
        Vec3::new(0.0, 1.0, 0.0),
    );
    let tris = volume(ObstacleShape::Capsule, pose).triangles();
    for v in tris.iter().flatten() {
        assert!(
            (v.y - 0.0).abs() < 1e-5 || (v.y - 2.0).abs() < 1e-5,
            "upright"
        );
        let r = (v.x * v.x + v.z * v.z).sqrt();
        assert!(r == 0.0 || r >= 1.0 - 1e-5, "circumscribes radius 0.5 × 2");
    }
}

#[test]
fn displacement_is_how_far_the_shape_moved() {
    let a = volume(ObstacleShape::Box, Mat4::IDENTITY);
    let b = volume(ObstacleShape::Box, Mat4::from_translation(Vec3::X * 0.3));
    assert!((a.displacement(&b) - 0.3).abs() < 1e-5);
    let turned = volume(ObstacleShape::Box, Mat4::from_rotation_y(0.2));
    assert!(
        a.displacement(&turned) > 0.3,
        "turning moves the far corners"
    );
}

#[test]
fn the_tick_tracks_velocity_and_stationary_time() {
    let mut scene = Scene::new();
    let id = add_obstacle(&mut scene, Vec3::ZERO, Vec3::ONE, true);
    let read = |s: &Scene| s.world.nav_obstacle(id).map(|o| o.clone()).unwrap();
    tick_obstacles(&mut scene, 0.5);
    let o = read(&scene);
    assert_eq!(
        o.stationary_time, o.time_to_stationary,
        "first seen: already still"
    );
    move_to(&mut scene, id, Vec3::new(1.0, 0.0, 0.0));
    tick_obstacles(&mut scene, 0.5);
    let o = read(&scene);
    assert_eq!(o.velocity, Vec3::new(2.0, 0.0, 0.0));
    assert_eq!(o.stationary_time, 0.0, "moved past the threshold");
    move_to(&mut scene, id, Vec3::new(1.05, 0.0, 0.0));
    tick_obstacles(&mut scene, 0.5);
    assert_eq!(
        read(&scene).stationary_time,
        0.5,
        "under the threshold counts as still"
    );
}

#[test]
fn only_non_carving_obstacles_are_avoided() {
    let mut scene = Scene::new();
    let carving = add_obstacle(&mut scene, Vec3::ZERO, Vec3::ONE, true);
    let plain = add_obstacle(
        &mut scene,
        Vec3::new(4.0, 0.0, 0.0),
        Vec3::new(2.0, 1.0, 2.0),
        false,
    );
    let discs = avoidance_obstacles(&scene);
    assert_eq!(discs.len(), 1);
    assert_eq!(discs[0].position, Vec2::new(4.0, 0.0));
    assert!(
        (discs[0].radius - 2f32.sqrt()).abs() < 1e-5,
        "the box's corner circle"
    );
    assert!(!discs[0].solves);
    let carved = carving_volumes(&scene);
    assert_eq!(
        carved.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
        vec![carving]
    );
    let _ = plain;
}

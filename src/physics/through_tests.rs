//! src/physics/through_tests.rs — `raycast_through` (#830): enter/exit pairs for a
//! box, a mesh crossed twice, a ray ending or starting inside, and triggers.

use glam::Vec3;
use rapier3d::prelude::*;

use super::through::spans;
use super::PhysicsWorld;
use crate::components::{ColliderComponent, ColliderShape};
use crate::scene::Scene;

/// A static 2 m box entity centered at `pos`, a trigger when `trigger`.
fn add_box(scene: &mut Scene, pos: Vec3, trigger: bool) -> u32 {
    let id = scene.add_entity("Box".to_string());
    scene.world.transform_mut(id).unwrap().position = pos;
    scene.world.set_static(id, true);
    let collider = ColliderComponent {
        active: true,
        shape: ColliderShape::Box {
            size: Vec3::splat(2.0),
        },
        is_trigger: trigger,
        material: Default::default(),
        aabb_min: Vec3::ZERO,
        aabb_max: Vec3::ZERO,
    };
    scene.world.set_collider(id, Some(collider));
    id
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn a_box_reports_its_entry_exit_and_thickness() {
    let mut scene = Scene::new();
    let id = add_box(&mut scene, Vec3::new(0.0, 0.0, 5.0), false);
    let world = PhysicsWorld::from_scene(&scene);
    let [c] = world.raycast_through(Vec3::ZERO, Vec3::Z, 100.0, |_| true)[..] else {
        panic!("one crossing");
    };
    let exit = c.exit.expect("the ray leaves the box");
    assert_eq!((c.enter.id, exit.id), (id, id));
    assert!(
        near(c.enter.distance, 4.0) && near(exit.distance, 6.0),
        "{c:?}"
    );
    assert!(near(c.thickness, 2.0), "{c:?}");
    assert!((exit.point - Vec3::new(0.0, 0.0, 6.0)).length() < 1e-3);
    assert_eq!((c.enter.normal, exit.normal), (-Vec3::Z, Vec3::Z));
}

#[test]
fn a_ray_ending_inside_has_no_exit_and_measures_to_max_distance() {
    let mut scene = Scene::new();
    add_box(&mut scene, Vec3::new(0.0, 0.0, 5.0), false);
    let world = PhysicsWorld::from_scene(&scene);
    let c = world.raycast_through(Vec3::ZERO, Vec3::Z, 4.5, |_| true)[0];
    assert_eq!(c.exit, None);
    assert!(near(c.thickness, 0.5), "{c:?}");
    // A huge max distance still finds the exact exit.
    let far = world.raycast_through(Vec3::ZERO, Vec3::Z, f32::MAX, |_| true)[0];
    assert!(near(far.exit.unwrap().distance, 6.0), "{far:?}");
}

#[test]
fn a_ray_starting_inside_enters_at_zero_facing_back() {
    let mut scene = Scene::new();
    add_box(&mut scene, Vec3::ZERO, false);
    let world = PhysicsWorld::from_scene(&scene);
    let c = world.raycast_through(Vec3::ZERO, Vec3::X, 100.0, |_| true)[0];
    assert_eq!((c.enter.distance, c.enter.point), (0.0, Vec3::ZERO));
    assert_eq!(c.enter.normal, -Vec3::X);
    assert!(near(c.exit.unwrap().distance, 1.0) && near(c.thickness, 1.0));
}

#[test]
fn triggers_cross_like_solids_and_the_filter_drops_them() {
    let mut scene = Scene::new();
    let wall = add_box(&mut scene, Vec3::new(0.0, 0.0, 9.0), false);
    let smoke = add_box(&mut scene, Vec3::new(0.0, 0.0, 3.0), true);
    let world = PhysicsWorld::from_scene(&scene);
    let ids = |accept: &dyn Fn(u32) -> bool| -> Vec<u32> {
        let all = world.raycast_through(Vec3::ZERO, Vec3::Z, 100.0, accept);
        all.iter().map(|c| c.enter.id).collect()
    };
    assert_eq!(ids(&|_| true), [smoke, wall]);
    assert_eq!(ids(&|id| id != smoke), [wall]);
}

/// Two closed 2 m cubes, outward-wound, as one mesh: centers at z = 3 and 9.
fn two_room_mesh() -> TriMesh {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for center in [Vec3::new(0.0, 0.0, 3.0), Vec3::new(0.0, 0.0, 9.0)] {
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            for side in [axis, -axis] {
                let u = side.any_orthonormal_vector();
                let v = side.cross(u);
                let base = vertices.len() as u32;
                for (a, b) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                    vertices.push(center + side + u * a + v * b);
                }
                // u × v = side, so (0,1,2) winds counter-clockwise seen from outside.
                indices.push([base, base + 1, base + 2]);
                indices.push([base, base + 2, base + 3]);
            }
        }
    }
    TriMesh::new(vertices, indices).unwrap()
}

#[test]
fn a_mesh_crossed_twice_yields_two_spans() {
    let mesh = two_room_mesh();
    let pose = Pose::IDENTITY;
    let ray = Ray::new(Vec3::ZERO, Vec3::Z);
    let got: Vec<_> = spans(&mesh, &pose, &ray, 100.0)
        .iter()
        .map(|s| (s.enter, s.exit.map(|e| e.0)))
        .collect();
    assert_eq!(got.len(), 2, "{got:?}");
    for ((enter, exit), want) in got.iter().zip([(2.0, 4.0), (8.0, 10.0)]) {
        assert!(
            near(*enter, want.0) && near(exit.unwrap(), want.1),
            "{got:?}"
        );
    }
    // From inside the first cube: enter at 0, then the second cube in full.
    let inside = Ray::new(Vec3::new(0.0, 0.0, 3.0), Vec3::Z);
    let got = spans(&mesh, &pose, &inside, 100.0);
    assert_eq!(got.len(), 2, "{got:?}");
    assert_eq!((got[0].enter, got[0].enter_normal), (0.0, -Vec3::Z));
    assert!(near(got[0].exit.unwrap().0, 1.0) && near(got[1].enter, 5.0));
    // Ending inside the second cube: its exit is missing.
    let short = spans(&mesh, &pose, &ray, 9.0);
    assert_eq!(short.len(), 2);
    assert_eq!(short[1].exit, None);
}

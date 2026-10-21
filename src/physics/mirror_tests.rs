//! src/physics/mirror_tests.rs — colliders under a mirrored parent (#1124).
//!
//! A negative world scale (glam puts the reflection in `scale.x`) keeps every
//! primitive at its drawn size, Unity-style, and turns a mesh collider's geometry
//! without turning its triangles inside out. Asserts the exact distance a ray
//! travels to each shape, so a collapsed axis or a halved radius is caught.

use glam::Vec3;

use super::build::build_shape;
use super::PhysicsWorld;
use crate::components::{CapsuleAxis, ColliderComponent, ColliderShape};
use crate::scene::Scene;

/// The mirrors under test: a pure reflection, and a reflection with a stretch.
const MIRRORS: [f32; 2] = [-1.0, -2.0];

/// A static `shape` at local `pos` under `parent`.
fn add_child(scene: &mut Scene, parent: u32, pos: Vec3, shape: ColliderShape) -> u32 {
    let id = scene.add_entity("Child".to_string());
    scene.set_parent(id, Some(parent)).unwrap();
    scene.world.transform_mut(id).unwrap().position = pos;
    scene.world.set_static(id, true);
    let collider = ColliderComponent {
        shape,
        ..crate::scene::authoring::default_collider()
    };
    scene.world.set_collider(id, Some(collider));
    id
}

/// Distance a ray from `x = -10` along +X at height `y` and depth `z` travels to `id`.
fn hit_from_minus_x(world: &PhysicsWorld, id: u32, y: f32, z: f32) -> f32 {
    let hit = world
        .raycast_hit(Vec3::new(-10.0, y, z), Vec3::X, 50.0, |_| true)
        .expect("the ray hits a shape");
    assert_eq!(hit.id, id, "the ray at y {y}, z {z} hits the wrong shape");
    hit.distance
}

#[test]
fn mirrored_primitives_keep_their_drawn_size() {
    for sx in MIRRORS {
        let (mirror, abs) = (Vec3::new(sx, 1.0, 1.0), Vec3::new(-sx, 1.0, 1.0));
        let shapes = [
            ColliderShape::Box { size: Vec3::ONE },
            ColliderShape::Sphere { radius: 0.5 },
            ColliderShape::Cylinder {
                radius: 0.5,
                height: 2.0,
            },
            ColliderShape::Capsule {
                radius: 0.25,
                height: 2.0,
                axis: CapsuleAxis::X,
            },
        ];
        for shape in shapes {
            let got = build_shape(&shape, mirror, None).unwrap();
            let want = build_shape(&shape, abs, None).unwrap();
            let (g, w) = (
                got.shape().compute_local_aabb(),
                want.shape().compute_local_aabb(),
            );
            assert_eq!(
                (g.mins, g.maxs),
                (w.mins, w.maxs),
                "{shape:?} under {mirror}"
            );
        }
    }
}

#[test]
fn a_ray_hits_mirrored_shapes_where_they_are_drawn() {
    for sx in MIRRORS {
        let mut scene = Scene::new();
        let parent = scene.add_entity("Mirror".to_string());
        scene.world.transform_mut(parent).unwrap().scale = Vec3::new(sx, 1.0, 1.0);
        let cube = add_child(
            &mut scene,
            parent,
            Vec3::Z * 3.0,
            ColliderShape::Box { size: Vec3::ONE },
        );
        let ball = add_child(
            &mut scene,
            parent,
            Vec3::Z * -3.0,
            ColliderShape::Sphere { radius: 0.5 },
        );
        let capsule = ColliderShape::Capsule {
            radius: 0.25,
            height: 2.0,
            axis: CapsuleAxis::X,
        };
        let pill = add_child(&mut scene, parent, Vec3::Y * 3.0, capsule);
        let world = PhysicsWorld::from_scene(&scene);
        let s = -sx;
        // Each shape sits on the X = 0 plane, so its near face is at -half its scaled width.
        let cases = [
            (cube, 0.0, 3.0, 0.5 * s),
            (ball, 0.0, -3.0, 0.5 * s),
            (pill, 3.0, 0.0, s),
        ];
        for (id, y, z, half_width) in cases {
            let d = hit_from_minus_x(&world, id, y, z);
            assert!(
                (d - (10.0 - half_width)).abs() < 1e-3,
                "scale {sx}: shape {id} at {d}"
            );
        }
    }
}

/// Whether every triangle of a trimesh centred on the origin faces away from it.
fn faces_out(collider: &rapier3d::prelude::Collider) -> bool {
    let mesh = collider.shape().as_trimesh().expect("a trimesh");
    let v = mesh.vertices();
    mesh.indices().iter().all(|&[a, b, c]| {
        let (a, b, c) = (v[a as usize], v[b as usize], v[c as usize]);
        (b - a).cross(c - a).dot(a + b + c) > 0.0
    })
}

#[test]
fn a_mirrored_trimesh_faces_out() {
    // A regular tetrahedron centred on the origin, every face wound outward.
    let p = [
        [1.0, 1.0, 1.0],
        [1.0, -1.0, -1.0],
        [-1.0, 1.0, -1.0],
        [-1.0, -1.0, 1.0],
    ];
    let i = [0, 1, 2, 0, 3, 1, 0, 2, 3, 1, 3, 2];
    let shape = ColliderShape::Mesh {
        convex: false,
        local_min: Vec3::NEG_ONE,
        local_max: Vec3::ONE,
    };
    let mesh = Some((p.as_slice(), i.as_slice()));
    // Unscaled proves the fixture; two negative axes are a rotation, so the winding stays.
    for (x, y) in [(1.0, 1.0), (-1.0, 1.0), (-2.0, 1.0), (-1.0, -1.0)] {
        let scale = Vec3::new(x, y, 1.0);
        let built = build_shape(&shape, scale, mesh).unwrap();
        assert!(faces_out(&built), "trimesh under {scale} faces inward");
    }
}

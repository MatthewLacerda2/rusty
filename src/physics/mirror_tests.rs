//! src/physics/mirror_tests.rs — colliders under a mirrored parent (#1124).
//!
//! A negative world scale (see `MIRRORS` for where one lands) keeps every primitive
//! at its drawn size, Unity-style, and turns a mesh collider's geometry without
//! turning its triangles inside out. Exact ray distances catch a collapsed axis.

use glam::{vec3, Vec3};

use super::build::build_shape;
use super::build_tests::mesh_shape;
use super::PhysicsWorld;
use crate::components::{CapsuleAxis, ColliderComponent, ColliderShape};
use crate::scene::Scene;

/// The mirrors under test: on X, a pure reflection and one with a stretch (where a
/// child's lands); on Y and Z, where only an unparented entity's own scale puts one.
const MIRRORS: [Vec3; 4] = [
    vec3(-1.0, 1.0, 1.0),
    vec3(-2.0, 1.0, 1.0),
    vec3(1.0, -1.0, 1.0),
    vec3(1.0, 1.0, -2.0),
];

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
    for mirror in MIRRORS {
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
            let want = build_shape(&shape, mirror.abs(), None).unwrap();
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
    for sx in MIRRORS.map(|m| m.x).into_iter().filter(|&x| x < 0.0) {
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
    let shape = mesh_shape(false);
    let mesh = Some((p.as_slice(), i.as_slice()));
    // Unscaled proves the fixture; two negative axes are a rotation, so the winding stays.
    for (x, y) in [(1.0, 1.0), (-1.0, 1.0), (-2.0, 1.0), (-1.0, -1.0)] {
        let scale = Vec3::new(x, y, 1.0);
        let built = build_shape(&shape, scale, mesh).unwrap();
        assert!(faces_out(&built), "trimesh under {scale} faces inward");
    }
}

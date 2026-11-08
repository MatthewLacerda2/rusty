//! src/physics/shape/internal_edges_tests.rs — a body sliding over a tiled trimesh
//! floor does not hop at the seams (#1880).

use glam::Vec3;
use rapier3d::prelude::*;

use super::mesh::{build_mesh_shape, TRIMESH_FLAGS};

/// Tiles per side, each its own two triangles over four unshared vertices.
const TILES: usize = 16;
const DT: f32 = 1.0 / 60.0;

/// A `TILES`² floor of 1 m quads at y = 0, centred on the origin, laid like modular
/// tiles: no tile shares a vertex with its neighbour, so only a merge joins them.
fn tiled_floor() -> (Vec<[f32; 3]>, Vec<u32>) {
    let (mut positions, mut indices) = (Vec::new(), Vec::new());
    let half = TILES as f32 / 2.0;
    for i in 0..TILES {
        for j in 0..TILES {
            let (x, z) = (i as f32 - half, j as f32 - half);
            let base = positions.len() as u32;
            positions.extend([[x, 0.0, z], [x, 0.0, z + 1.0], [x + 1.0, 0.0, z + 1.0]]);
            positions.push([x + 1.0, 0.0, z]);
            indices.extend([base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    }
    (positions, indices)
}

/// The largest vertical speed (from its pose) `body` reaches sliding at `speed` along x over
/// `floor`, after a settling second, over three seconds of steps.
fn worst_hop(floor: Collider, body: Collider, height: f32, speed: f32) -> f32 {
    let (mut bodies, mut colliders) = (RigidBodySet::new(), ColliderSet::new());
    colliders.insert(floor);
    let start = RigidBodyBuilder::dynamic()
        .translation(Vector::new(-6.0, height, 0.3))
        .linvel(Vector::new(speed, 0.0, speed * 0.3))
        .build();
    let handle = bodies.insert(start);
    colliders.insert_with_parent(body, handle, &mut bodies);
    let mut parts = (
        IslandManager::new(),
        DefaultBroadPhase::new(),
        NarrowPhase::new(),
    );
    let (mut impulse, mut multibody) = (ImpulseJointSet::new(), MultibodyJointSet::new());
    let (mut ccd, mut pipeline) = (CCDSolver::new(), PhysicsPipeline::new());
    let params = IntegrationParameters {
        dt: DT,
        ..Default::default()
    };
    let (mut worst, mut last) = (0.0f32, height);
    for step in 0..180 {
        pipeline.step(
            Vector::new(0.0, -9.81, 0.0),
            &params,
            &mut parts.0,
            &mut parts.1,
            &mut parts.2,
            &mut bodies,
            &mut colliders,
            &mut impulse,
            &mut multibody,
            &mut SoftBodySet::new(),
            &mut ccd,
            &(),
            &(),
        );
        // Measured from the pose: a resting contact reports a small steady `linvel.y`.
        let y = bodies[handle].translation().y;
        if step >= 20 {
            worst = worst.max((y - last).abs() / DT);
        }
        last = y;
    }
    worst
}

/// The tiled floor, frictionless: built by the engine, or straight through rapier with
/// no trimesh flags, as every mesh collider was before #1880.
fn tiled(engine: bool) -> Collider {
    let (positions, indices) = tiled_floor();
    let mut floor = if engine {
        build_mesh_shape(false, Vec3::ONE, (&positions, &indices)).unwrap()
    } else {
        let points = positions.iter().map(|&p| Vector::from(p)).collect();
        let tris = indices.chunks_exact(3).map(|c| [c[0], c[1], c[2]]);
        ColliderBuilder::trimesh(points, tris.collect())
            .unwrap()
            .build()
    };
    floor.set_friction(0.0);
    floor
}

/// Above this, a body resting on a flat floor was kicked by a seam.
const HOP: f32 = 0.05;

/// `body` (resting at `height`) crosses the engine's floor flat and snags on an
/// unflagged one: the test fails with the flag off.
fn assert_no_hop(body: impl Fn() -> Collider, height: f32) {
    let fixed = worst_hop(tiled(true), body(), height, 4.0);
    assert!(fixed < HOP, "the engine's floor kicks it to {fixed} m/s");
    let raw = worst_hop(tiled(false), body(), height, 4.0);
    assert!(
        raw >= HOP,
        "an unflagged floor should snag it, got {raw} m/s"
    );
}

#[test]
fn a_box_slides_over_tile_seams_without_hopping() {
    assert_no_hop(
        || {
            ColliderBuilder::cuboid(0.25, 0.25, 0.25)
                .friction(0.0)
                .build()
        },
        0.25,
    );
}

#[test]
fn a_ball_rolls_over_tile_seams_without_hopping() {
    assert_no_hop(|| ColliderBuilder::ball(0.25).build(), 0.25);
}

#[test]
fn the_engine_builds_every_trimesh_with_the_flag() {
    assert!(TRIMESH_FLAGS.contains(TriMeshFlags::FIX_INTERNAL_EDGES));
    let flags = tiled(true).shape().as_trimesh().unwrap().flags();
    assert_eq!(flags, TRIMESH_FLAGS);
}

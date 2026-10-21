//! Property tests for the world-space store and API (#1170), over random hierarchies up
//! to depth 4 with uniform, non-uniform, one-mirrored-axis, one-zero-axis and Dust2-size
//! scales. Seeded (`RngSeed::Fixed`), so every run draws the same cases.

use glam::{EulerRot, Mat3, Mat4, Quat, Vec3};
use proptest::prelude::*;
use proptest::test_runner::{Config, RngSeed};
use rusty::scene::authoring::default_collider;
use rusty::scene::{apply_scene_data, to_scene_data, Scene};

#[derive(Clone, Debug)]
struct Node {
    parent: usize,
    pos: Vec3,
    rot: Quat,
    scale: Vec3,
}

fn scale() -> impl Strategy<Value = Vec3> {
    let signed = |axis: usize, s: f32, v: f32| {
        let mut out = Vec3::splat(s);
        out[axis] = v;
        out
    };
    prop_oneof![
        (0.2f32..3.0).prop_map(Vec3::splat),
        prop::array::uniform3(0.2f32..3.0).prop_map(Vec3::from_array),
        (0usize..3, 0.2f32..3.0).prop_map(move |(a, s)| signed(a, s, -s)),
        (0usize..3, 0.2f32..3.0).prop_map(move |(a, s)| signed(a, s, 0.0)),
        (50.0f32..200.0).prop_map(Vec3::splat),
    ]
}

fn node() -> impl Strategy<Value = Node> {
    let angle = -3.2f32..3.2;
    (
        any::<usize>(),
        prop::array::uniform3(-50.0f32..50.0),
        (angle.clone(), angle.clone(), angle),
        scale(),
    )
        .prop_map(|(parent, pos, (a, b, c), scale)| Node {
            parent,
            pos: Vec3::from_array(pos),
            rot: Quat::from_euler(EulerRot::YXZ, a, b, c),
            scale,
        })
}

/// The nodes as a scene, each with a box collider; node `i` hangs under an earlier one
/// (or none) so no chain is deeper than 4.
fn build(nodes: &[Node]) -> (Scene, Vec<u32>) {
    let mut scene = Scene::new();
    let (mut ids, mut depth) = (Vec::new(), Vec::new());
    for (i, n) in nodes.iter().enumerate() {
        let id = scene.add_entity(format!("n{i}"));
        let mut t = scene.world.transform_mut(id).unwrap();
        (t.position, t.rotation, t.scale) = (n.pos, n.rot, n.scale);
        drop(t);
        scene.world.set_collider(id, Some(default_collider()));
        let parent = (i > 0).then(|| n.parent % (i + 1)).filter(|&p| p < i);
        let parent = parent.filter(|&p| depth[p] < 3);
        if let Some(p) = parent {
            scene.set_parent(id, Some(ids[p])).unwrap();
        }
        depth.push(parent.map_or(0, |p| depth[p] + 1));
        ids.push(id);
    }
    (scene, ids)
}

fn near(a: Mat4, b: Mat4) -> bool {
    let scale = 1.0
        + a.abs()
            .to_cols_array()
            .iter()
            .fold(0.0f32, |m, v| m.max(*v));
    a.abs_diff_eq(b, 1e-4 * scale)
}

/// Whether `m`'s axes are orthogonal and none is collapsed: a TRS can represent it.
fn trs(m: Mat4) -> bool {
    let c = Mat3::from_mat4(m);
    let (x, y, z) = (c.x_axis, c.y_axis, c.z_axis);
    let ortho = |a: Vec3, b: Vec3| a.dot(b).abs() <= 1e-4 * a.length() * b.length();
    let len = [x, y, z].map(|v| v.length());
    len.iter().all(|&l| l > 1e-3) && ortho(x, y) && ortho(y, z) && ortho(x, z)
}

fn config() -> Config {
    Config {
        cases: 128,
        rng_seed: RngSeed::Fixed(1170),
        ..Config::default()
    }
}

proptest! {
    #![proptest_config(config())]

    #[test]
    fn world_space_agrees_with_the_matrix(nodes in prop::collection::vec(node(), 1..8)) {
        let (scene, ids) = build(&nodes);
        let w = &scene.world;
        for &id in &ids {
            let m = w.local_to_world(id);
            let scratch = w
                .self_and_ancestors(id)
                .map(|c| w.transform(c).unwrap().to_matrix())
                .fold(Mat4::IDENTITY, |below, local| local * below);
            prop_assert!(near(m, scratch), "{id}: {m} vs {scratch}");
            let pose = w.world_pose(id).unwrap();
            prop_assert!(m.is_finite() && pose.pos.is_finite());
            prop_assert!(pose.rot.is_finite() && pose.scale.is_finite());
            prop_assert_eq!(w.is_mirrored(id), m.determinant() < 0.0);
            let p = Vec3::new(0.3, -1.2, 2.0);
            prop_assert_eq!(w.transform_point(id, p), m.transform_point3(p));
            if trs(m) {
                let back = Mat4::from_scale_rotation_translation(pose.scale, pose.rot, pose.pos);
                prop_assert!(near(back, m), "{id}: {back} vs {m}");
            }
        }
    }

    #[test]
    fn moving_an_ancestor_keeps_every_aabb_fresh(
        nodes in prop::collection::vec(node(), 2..8),
        mover in any::<usize>(),
        to in prop::array::uniform3(-20.0f32..20.0),
        turn in -3.0f32..3.0,
    ) {
        let (mut scene, ids) = build(&nodes);
        let moved = ids[mover % ids.len()];
        scene.world.set_world_position(moved, Vec3::from_array(to));
        scene.world.transform_mut(moved).unwrap().rotation *= Quat::from_rotation_y(turn);
        let mut fresh = Scene::new();
        apply_scene_data(&mut fresh, to_scene_data(&scene));
        let read = |scene: &Scene| ids.iter().map(|&id| scene.world.collider_bounds(id).unwrap()).collect::<Vec<_>>();
        let (before, want) = (read(&scene), read(&fresh));
        scene.world.flush_bounds();
        // Read mid-stage (worked out from the pose) and after the flush (stored).
        for (got, when) in [(before, "read"), (read(&scene), "flushed")] {
            for (&(a_min, a_max), &(b_min, b_max)) in got.iter().zip(&want) {
                let tol = 1e-4 * (1.0 + a_max.abs().max_element() + a_min.abs().max_element());
                prop_assert!(a_min.abs_diff_eq(b_min, tol), "{when}: {a_min} vs {b_min}");
                prop_assert!(a_max.abs_diff_eq(b_max, tol), "{when}: {a_max} vs {b_max}");
            }
        }
    }
}

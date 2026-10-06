//! How a vertex's weights file it into the animated bound (#893): weight on any
//! one slot skins it, and a zero weight adds nothing to the joint it names.

use glam::{Mat4, Vec3};

use super::SkinBounds;
use crate::components::mesh::Vertex;

/// A vertex at `p` weighted to `joints` by `weights`.
fn vertex(p: Vec3, joints: [u32; 4], weights: [f32; 4]) -> Vertex {
    let mut v = Vertex::new(p, Vec3::Y, [0.0, 0.0]);
    v.joint_indices = joints;
    v.joint_weights = weights;
    v
}

#[test]
fn a_vertex_weighted_on_any_one_slot_is_skinned() {
    // Vertex k weighs 1.0 on slot k alone, so the four slots each carry one.
    let corners = [Vec3::ZERO, Vec3::X, Vec3::Y, Vec3::Z];
    let vertices: Vec<Vertex> = (0..4)
        .map(|k| {
            let mut w = [0.0; 4];
            w[k] = 1.0;
            vertex(corners[k], [0, 1, 2, 3], w)
        })
        .collect();
    let palette = [Mat4::from_translation(Vec3::X * 10.0); 4];
    // Every vertex moved with its joint: nothing is left at its bind position.
    assert_eq!(
        SkinBounds::from_vertices(&vertices).posed(&palette),
        Some((Vec3::new(10.0, 0.0, 0.0), Vec3::new(11.0, 1.0, 1.0)))
    );
}

#[test]
fn a_zero_weight_does_not_grow_its_joints_box() {
    let p = Vec3::new(1.0, 2.0, 3.0);
    let vertices = [vertex(p, [0, 5, 0, 0], [1.0, 0.0, 0.0, 0.0])];
    let mut palette = [Mat4::IDENTITY; 6];
    palette[5] = Mat4::from_translation(Vec3::X * 100.0);
    assert_eq!(
        SkinBounds::from_vertices(&vertices).posed(&palette),
        Some((p, p)),
        "joint 5 carries no weight, so its pose is irrelevant"
    );
}

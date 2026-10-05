//! The Cylinder primitive: a tube between two points plus its two disk caps, with a
//! generated lightmap UV (#831).

use glam::Vec3;

use super::fill_tangents_unwrapped;
use crate::components::mesh::Vertex;

/// Generates a cylinder between two arbitrary 3D points
pub fn generate_cylinder(
    p1: Vec3,
    p2: Vec3,
    radius: f32,
    segments: u32,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    let d_vec = p2 - p1;
    let dir = d_vec.normalize();

    // Create orthonormal basis (dir, u, v)
    let u = if dir.x.abs() < 0.9 {
        dir.cross(Vec3::X).normalize()
    } else {
        dir.cross(Vec3::Y).normalize()
    };
    let v = dir.cross(u).normalize();

    // 1. Tube wall, 2. bottom cap (−dir at p1), 3. top cap (+dir at p2).
    push_cylinder_tube(
        &mut vertices,
        &mut indices,
        (p1, p2),
        (u, v),
        radius,
        segments,
    );
    push_cylinder_cap(
        &mut vertices,
        &mut indices,
        (p1, -dir, true),
        (u, v),
        radius,
        segments,
    );
    push_cylinder_cap(
        &mut vertices,
        &mut indices,
        (p2, dir, false),
        (u, v),
        radius,
        segments,
    );

    fill_tangents_unwrapped(vertices, indices)
}

/// Append the cylinder's side wall: paired bottom/top ring vertices and the
/// triangles connecting consecutive segments.
fn push_cylinder_tube(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    ends: (Vec3, Vec3),
    basis: (Vec3, Vec3),
    radius: f32,
    segments: u32,
) {
    let (p1, p2) = ends;
    let (u, v) = basis;
    for i in 0..=segments {
        let theta = (i as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
        let radial_dir = u * theta.cos() + v * theta.sin();
        let p_offset = radial_dir * radius;
        let uv_x = (i as f32) / (segments as f32);
        // Bottom vertex (on P1 plane), then top vertex (on P2 plane).
        vertices.push(Vertex::new(p1 + p_offset, radial_dir, [uv_x, 0.0]));
        vertices.push(Vertex::new(p2 + p_offset, radial_dir, [uv_x, 1.0]));
    }
    for i in 0..segments {
        let (b0, t0, b1, t1) = (i * 2, i * 2 + 1, (i + 1) * 2, (i + 1) * 2 + 1);
        indices.extend_from_slice(&[b0, b1, t0, t0, b1, t1]);
    }
}

/// Append one disk cap. `cap` is `(center, outward normal, flip_winding)`: the bottom
/// cap winds (center, i+1, i) so the face points outward, the top cap (center, i, i+1).
fn push_cylinder_cap(
    vertices: &mut Vec<Vertex>,
    indices: &mut Vec<u32>,
    cap: (Vec3, Vec3, bool),
    basis: (Vec3, Vec3),
    radius: f32,
    segments: u32,
) {
    let (center, normal, flip) = cap;
    let (u, v) = basis;
    let center_idx = vertices.len() as u32;
    vertices.push(Vertex::new(center, normal, [0.5, 0.5]));
    let ring_start = vertices.len() as u32;
    for i in 0..=segments {
        let theta = (i as f32) * 2.0 * std::f32::consts::PI / (segments as f32);
        let (cos_t, sin_t) = (theta.cos(), theta.sin());
        let radial_dir = u * cos_t + v * sin_t;
        let p_offset = radial_dir * radius;
        let uv = [0.5 + 0.5 * cos_t, 0.5 + 0.5 * sin_t];
        vertices.push(Vertex::new(center + p_offset, normal, uv));
    }
    for i in 0..segments {
        if flip {
            indices.extend_from_slice(&[center_idx, ring_start + i + 1, ring_start + i]);
        } else {
            indices.extend_from_slice(&[center_idx, ring_start + i, ring_start + i + 1]);
        }
    }
}

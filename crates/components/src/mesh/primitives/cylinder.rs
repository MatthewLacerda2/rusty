use glam::Vec3;

use super::fill_tangents_unwrapped;
use crate::mesh::Vertex;

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

    let u = if dir.x.abs() < 0.9 {
        dir.cross(Vec3::X).normalize()
    } else {
        dir.cross(Vec3::Y).normalize()
    };
    let v = dir.cross(u).normalize();

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
        vertices.push(Vertex::new(p1 + p_offset, radial_dir, [uv_x, 0.0]));
        vertices.push(Vertex::new(p2 + p_offset, radial_dir, [uv_x, 1.0]));
    }
    for i in 0..segments {
        let (b0, t0, b1, t1) = (i * 2, i * 2 + 1, (i + 1) * 2, (i + 1) * 2 + 1);
        indices.extend_from_slice(&[b0, b1, t0, t0, b1, t1]);
    }
}

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
